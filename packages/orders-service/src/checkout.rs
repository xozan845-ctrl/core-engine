//! `CreateOrderCommandHandler`: valida las reglas (RN-01/03/04/05), persiste el
//! evento y el outbox en la misma transacción y devuelve la vista de la orden.
//! Además aplica transiciones (logística → orders) y replay.

use crate::carrito::{Carrito, ItemCarrito};
use crate::clients::{
    ClientesPort, ConfiguracionExterna, ConfiguracionFiscal, ConfiguracionOperativa, CreditoEstado,
    CuponExterno, OfertaExterna, PreciosExternos, PromocionLinea,
};
use crate::db::{iso_ms, Db};
use crate::errors::ApiError;
use crate::event_store::{error_db, parsear_uuid, OrderEventStore};
use crate::fidelizacion::Fidelizacion;
use crate::fulfillments::Fulfillments;
use crate::money;
use crate::order_state::ItemOrden;
use crate::outbox::Outbox;
use crate::pagos::Pagos;
use crate::pedidos::Pedidos;
use crate::tipos_orden::TiposOrden;
use crate::views::{OrderView, OrderViewRepository};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

pub const ORDER_CREATED: &str = "order.created";
pub const ORDER_STATUS_UPDATED: &str = "order.status.updated";
pub const ORDER_CANCELLED: &str = "order.cancelled";
pub const PAYMENT_PROCESADO: &str = "payment.procesado";
pub const DEVOLUCION_SOLICITADA: &str = "devolucion.solicitada";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineaEntrada {
    pub oferta_id: String,
    pub cantidad: i64,
    #[serde(default)]
    pub fulfillment: Option<String>,
    #[serde(default)]
    pub fulfillment_config: Option<Value>,
    #[serde(default)]
    pub ubicacion_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ComandoCrear {
    #[serde(default)]
    pub cliente_parte_id: Option<String>,
    #[serde(default)]
    pub tipo_orden: String,
    #[serde(default)]
    pub items: Option<Vec<LineaEntrada>>,
    #[serde(default)]
    pub usar_carrito: bool,
    #[serde(default)]
    pub metodo_pago: Option<String>,
    #[serde(default)]
    pub moneda_pago: Option<String>,
    #[serde(default)]
    pub lista_precio_codigo: Option<String>,
    #[serde(default)]
    pub segmento_codigo: Option<String>,
    #[serde(default)]
    pub cupon_codigo: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct LineaAgrupada {
    oferta_id: String,
    cantidad: i64,
    fulfillment: Option<String>,
    fulfillment_config: Option<Value>,
    ubicacion_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct PrecioResuelto {
    origen: String,
    precio_unitario_cents: i64,
    promocion: Option<PromocionLinea>,
}

#[derive(Debug, Clone, PartialEq)]
struct EntregaPlan {
    empresa_id: String,
    item_id: String,
    sku: String,
    comprador_id: String,
    cliente_parte_id: Option<String>,
    recurso_url: String,
    token: String,
    max_descargas: Option<i64>,
    expira_en: Option<chrono::DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EstadoDerivado {
    pub order_id: String,
    pub comprador_id: String,
    pub items: Vec<ItemOrden>,
    pub total_cents: i64,
    pub estado: String,
    pub motivo: Option<String>,
    pub creado_en: String,
    pub actualizado_en: String,
    pub tipo_orden: String,
}

pub struct Checkout {
    pub db: Arc<Db>,
    pub clientes: Arc<dyn ClientesPort>,
    pub carrito: Arc<Carrito>,
    pub vistas: Arc<OrderViewRepository>,
    pub pedidos: Arc<Pedidos>,
    pub outbox: Outbox,
    pub pagos: Arc<Pagos>,
    pub tipos: Arc<TiposOrden>,
    pub fulfillments: Arc<Fulfillments>,
    pub tasa_comision: f64,
}

impl Checkout {
    #[allow(clippy::too_many_arguments)]
    pub async fn ejecutar(
        &self,
        comando: &ComandoCrear,
        cliente_id: &str,
    ) -> Result<OrderView, ApiError> {
        if comando.tipo_orden.trim().is_empty() {
            return Err(ApiError::dominio(
                "TIPO_ORDEN_REQUERIDO",
                "El tipo de orden es obligatorio.",
            ));
        }
        let tipo_orden = self.tipos.validar_tipo(&comando.tipo_orden).await?;
        let es_cotizacion = tipo_orden == "cotizacion";
        let es_reserva = tipo_orden == "reserva";
        let metodo_pago = comando
            .metodo_pago
            .as_deref()
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .unwrap_or("simulado")
            .to_lowercase();
        let fulfillments_permitidos = self.fulfillments.codigos_activos().await?;
        let metodo = self
            .pagos
            .metodo_activo(&metodo_pago)
            .await?
            .ok_or_else(|| {
                ApiError::dominio(
                    "METODO_PAGO_INVALIDO",
                    format!("El metodo de pago \"{metodo_pago}\" no existe o esta inactivo."),
                )
            })?;
        if es_reserva && comando.cliente_parte_id.is_none() {
            return Err(ApiError::dominio(
                "PARTE_REQUERIDA",
                "Una reserva requiere cliente_parte_id.",
            ));
        }
        if metodo_pago == "credito" && comando.cliente_parte_id.is_none() {
            return Err(ApiError::dominio(
                "CREDITO_REQUIERE_PARTE",
                "La venta a credito requiere identificar al cliente (cliente_parte_id).",
            ));
        }

        let mut desde_carrito = false;
        let mut lineas: Vec<LineaEntrada> = comando.items.clone().unwrap_or_default();
        if comando.usar_carrito {
            let del_carrito = self.carrito.items_para_checkout(cliente_id).await?;
            if del_carrito.is_empty() {
                return Err(ApiError::dominio(
                    "CARRITO_VACIO",
                    "El carrito esta vacio o expiro (RN-05).",
                ));
            }
            lineas = lineas_de_carrito(&del_carrito);
            desde_carrito = true;
        }
        if lineas.is_empty() {
            return Err(ApiError::dominio(
                "CARRITO_VACIO",
                "El carrito no puede estar vacio (RN-05).",
            ));
        }
        let agrupados = agrupar(&lineas)?;

        let ids: Vec<String> = agrupados.iter().map(|l| l.oferta_id.clone()).collect();
        let ofertas = self.clientes.ofertas(&ids).await;
        let por_id: HashMap<String, OfertaExterna> =
            ofertas.into_iter().map(|o| (o.id.clone(), o)).collect();
        let precios = self.resolver_precios(&agrupados, &por_id, comando).await?;

        let mut items: Vec<ItemOrden> = Vec::new();
        let mut url_digital: HashMap<String, String> = HashMap::new();
        let mut moneda_orden: Option<String> = None;
        let mut total: i64 = 0;

        for linea in &agrupados {
            let Some(oferta) = por_id.get(&linea.oferta_id) else {
                return Err(ApiError::no_encontrado("Oferta", &linea.oferta_id));
            };
            if oferta.estado != "activa" {
                return Err(ApiError::no_encontrado("Oferta", &linea.oferta_id));
            }
            let moneda_oferta = oferta
                .moneda
                .clone()
                .unwrap_or_else(|| "NIO".to_string())
                .to_uppercase();
            if let Some(actual) = &moneda_orden {
                if actual != &moneda_oferta {
                    return Err(ApiError::dominio(
                        "MONEDA_MIXTA",
                        "Una orden no puede mezclar ofertas en monedas distintas.",
                    ));
                }
            }
            moneda_orden = Some(moneda_oferta);
            let fulfillment = validar_fulfillment(
                linea.fulfillment.as_deref(),
                &tipo_orden,
                &oferta.item_type,
                &fulfillments_permitidos,
            )?;
            if fulfillment == "digital" {
                let url = url_entrega_digital(oferta.atributos.as_ref());
                let Some(url) = url else {
                    return Err(ApiError::dominio(
                        "ENTREGA_DIGITAL_NO_CONFIGURADA",
                        "El item digital no tiene configurada su URL de entrega.",
                    ));
                };
                url_digital.insert(oferta.id.clone(), url);
            }
            let mut precio_unitario = money::parsear(&oferta.precio_venta)?;
            if let Some(resuelto) = precios.get(&oferta.id) {
                if resuelto.origen != "base" {
                    precio_unitario =
                        money::aplicar_margen(resuelto.precio_unitario_cents, oferta.margen)?;
                }
            }
            let bruto = money::multiplicar(precio_unitario, linea.cantidad);
            let promocion = precios
                .get(&oferta.id)
                .and_then(|resuelto| resuelto.promocion.as_ref());
            let descuento = if tipo_orden == "reserva" {
                0
            } else {
                descuento_de_promocion(promocion, precio_unitario, linea.cantidad)?
            };
            total += bruto.saturating_sub(descuento);
            items.push(ItemOrden {
                oferta_id: oferta.id.clone(),
                sku: oferta.sku.clone(),
                producto_nombre: oferta.producto_nombre.clone(),
                cantidad: linea.cantidad,
                precio_unitario_cents: precio_unitario,
                descuento_cents: descuento,
                impuesto_cents: 0,
                tasa_impuesto_bps: 0,
                total_linea_cents: 0,
                promocion_id: promocion.map(|promo| promo.id.clone()),
                vendedor_id: oferta.vendedor_id.clone(),
                tienda_id: oferta.tienda_id.clone(),
                empresa_id: oferta.empresa_id.clone(),
                negocio_id: oferta.negocio_id.clone(),
                participacion_porcentaje: oferta.participacion_porcentaje,
                comision: true,
                ubicacion_id: linea.ubicacion_id.clone(),
                item_type: oferta.item_type.clone(),
                item_id: oferta.item_id.clone(),
                fulfillment: Some(fulfillment.clone()),
                fulfillment_config: linea.fulfillment_config.clone(),
            });
        }

        if metodo_pago == "credito" {
            let empresas: HashSet<&str> = items.iter().map(|i| i.empresa_id.as_str()).collect();
            if empresas.len() > 1 {
                return Err(ApiError::dominio(
                    "CREDITO_UNA_EMPRESA",
                    "Una venta a credito solo puede incluir items de una empresa.",
                ));
            }
        }

        let configuraciones = self
            .validar_capacidades(&items, &tipo_orden, &metodo_pago)
            .await?;
        for item in &mut items {
            item.comision = configuraciones
                .get(&item.empresa_id)
                .map(|config| config.comision)
                .unwrap_or(true);
        }

        if es_reserva {
            total = 0;
            for (indice, item) in items.iter_mut().enumerate() {
                let linea = &agrupados[indice];
                let config = linea
                    .fulfillment_config
                    .as_ref()
                    .and_then(Value::as_object)
                    .cloned()
                    .unwrap_or_default();
                let recurso_id = texto_config(&config, "recurso_id");
                let inicia_en = texto_config(&config, "inicia_en");
                let termina_en = texto_config(&config, "termina_en");
                let (Some(recurso_id), Some(inicia_en), Some(termina_en)) =
                    (recurso_id, inicia_en, termina_en)
                else {
                    return Err(ApiError::dominio(
                        "RESERVA_CONFIGURACION_REQUERIDA",
                        "Una reserva requiere recurso_id, inicia_en y termina_en.",
                    ));
                };
                if item.cantidad != 1 {
                    return Err(ApiError::dominio(
                        "RESERVA_CANTIDAD_INVALIDA",
                        "Una reserva admite cantidad 1.",
                    ));
                }
                let desde = inicia_en.get(0..10).unwrap_or("");
                let hasta = termina_en.get(0..10).unwrap_or("");
                if !es_fecha(desde) || !es_fecha(hasta) {
                    return Err(ApiError::dominio(
                        "RESERVA_FECHAS_INVALIDAS",
                        "Usa fechas ISO con hora en la reserva.",
                    ));
                }
                let cotizacion = self
                    .clientes
                    .cotizacion(&item.empresa_id, &item.item_id, desde, hasta)
                    .await
                    .ok_or_else(|| ApiError::servicio_no_disponible("pricing-service"))?;
                let disponible = self
                    .clientes
                    .disponibilidad(&item.empresa_id, &recurso_id, &inicia_en, &termina_en)
                    .await;
                match disponible {
                    Some(false) => {
                        return Err(ApiError::conflicto(
                            "El recurso no esta disponible en ese rango.",
                        )
                        .con_detalles(json!({ "recurso_id": recurso_id }))
                        .con_codigo("RESERVA_NO_DISPONIBLE"));
                    }
                    None => return Err(ApiError::servicio_no_disponible("scheduling-service")),
                    Some(true) => {}
                }
                item.precio_unitario_cents = cotizacion.total_cents;
                item.descuento_cents = 0;
                item.promocion_id = None;
                total += money::multiplicar(cotizacion.total_cents, item.cantidad);
            }
        }

        if !es_cotizacion {
            self.verificar_stock(&items).await?;
        }

        let total_bruto = total;
        let mut cupon_aplicado: Option<(String, String, i64)> = None;
        let codigo_cupon = comando
            .cupon_codigo
            .as_deref()
            .map(str::trim)
            .filter(|codigo| !codigo.is_empty());
        if let Some(codigo) = codigo_cupon {
            let empresa = empresa_unica(&items).ok_or_else(|| {
                ApiError::dominio(
                    "CUPON_UNA_EMPRESA",
                    "Un cupón solo puede aplicarse a una orden de una empresa.",
                )
            })?;
            match self
                .clientes
                .cupon_validar(&empresa, codigo, total_bruto)
                .await
            {
                CuponExterno::Ok(validado) => {
                    let descuento = validado.descuento_cents.clamp(0, total_bruto);
                    cupon_aplicado = Some((empresa, validado.codigo, descuento));
                }
                CuponExterno::Invalido { codigo, mensaje } => {
                    return Err(ApiError::dominio(codigo, mensaje));
                }
                CuponExterno::NoDisponible => {
                    return Err(ApiError::servicio_no_disponible("pricing-service"));
                }
            }
        }
        let mut descuento_cupon = cupon_aplicado
            .as_ref()
            .map(|(_, _, descuento)| *descuento)
            .unwrap_or(0);
        let cupon_codigo = cupon_aplicado.as_ref().map(|(_, codigo, _)| codigo.clone());

        let order_id = Uuid::new_v4().to_string();
        let creado_en = iso_ms(Utc::now());

        if let Some((empresa, codigo, _)) = &cupon_aplicado {
            let Some(validado) = self
                .clientes
                .cupon_canjear(empresa, codigo, &order_id, total_bruto)
                .await
            else {
                return Err(ApiError::servicio_no_disponible("pricing-service"));
            };
            descuento_cupon = validado.descuento_cents.clamp(0, total_bruto);
        }

        let (total_con_impuestos, impuesto_total) = self
            .calcular_impuestos(&mut items, descuento_cupon, total_bruto)
            .await?;
        total = total_con_impuestos;

        if metodo_pago == "credito" {
            let empresa_id = items
                .first()
                .map(|i| i.empresa_id.clone())
                .unwrap_or_default();
            let parte_id = comando.cliente_parte_id.as_deref().unwrap_or_default();
            let concepto = format!("Pedido {}", &order_id[..8]);
            let credito = self
                .clientes
                .registrar_credito(
                    &empresa_id,
                    parte_id,
                    &order_id,
                    total,
                    &concepto,
                    items.first().and_then(|i| i.vendedor_id.as_deref()),
                )
                .await;
            match credito {
                CreditoEstado::Rechazado => {
                    return Err(ApiError::nueva(
                        "CREDITO_RECHAZADO",
                        "La parte no tiene una cuenta de credito activa o cupo disponible.",
                        axum::http::StatusCode::CONFLICT,
                    ));
                }
                CreditoEstado::Error => {
                    return Err(ApiError::servicio_no_disponible("finance-service"));
                }
                CreditoEstado::Ok => {}
            }
        }

        let entregas = if es_cotizacion {
            Vec::new()
        } else {
            items
                .iter()
                .filter(|item| item.fulfillment.as_deref() == Some("digital"))
                .map(|item| {
                    let oferta = por_id.get(&item.oferta_id);
                    EntregaPlan {
                        empresa_id: item.empresa_id.clone(),
                        item_id: item.item_id.clone(),
                        sku: item.sku.clone(),
                        comprador_id: cliente_id.to_string(),
                        cliente_parte_id: comando.cliente_parte_id.clone(),
                        recurso_url: url_digital
                            .get(&item.oferta_id)
                            .cloned()
                            .unwrap_or_default(),
                        token: Uuid::new_v4().to_string(),
                        max_descargas: max_descargas_digital(
                            oferta.and_then(|o| o.atributos.as_ref()),
                        ),
                        expira_en: expira_digital(oferta.and_then(|o| o.atributos.as_ref())),
                    }
                })
                .collect()
        };

        let moneda_orden = moneda_orden.unwrap_or_else(|| "NIO".to_string());
        let moneda_pago = match comando
            .moneda_pago
            .as_deref()
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
        {
            Some(valor) => normalizar_moneda(valor)?,
            None => moneda_orden.clone(),
        };
        let total_pagado_cents = if moneda_pago == moneda_orden {
            total
        } else {
            let tasa = self
                .clientes
                .tipo_cambio(&moneda_orden, &moneda_pago)
                .await
                .ok_or_else(|| {
                    ApiError::dominio(
                        "TIPO_CAMBIO_NO_DISPONIBLE",
                        format!("No hay una tasa publicada de {moneda_orden} a {moneda_pago}."),
                    )
                })?;
            let monto = (total as f64 * tasa).round() as i64;
            if monto <= 0 {
                return Err(ApiError::dominio(
                    "TIPO_CAMBIO_INVALIDO",
                    "La tasa publicada produce un monto invalido.",
                ));
            }
            monto
        };
        let base_comision_bruta: i64 = if es_cotizacion {
            0
        } else {
            items
                .iter()
                .filter(|item| item.comision && item.vendedor_id.is_some())
                .map(|item| {
                    item.precio_unitario_cents
                        .saturating_mul(item.cantidad)
                        .saturating_sub(item.descuento_cents)
                })
                .sum()
        };
        let proporcion_comision = if total_bruto > 0 {
            base_comision_bruta as f64 / total_bruto as f64
        } else {
            0.0
        };
        let cupon_comision = (descuento_cupon as f64 * proporcion_comision).round() as i64;
        let base_comision = (base_comision_bruta - cupon_comision).max(0);
        let comision = if base_comision == 0 {
            0
        } else {
            money::comision(base_comision, self.tasa_comision)?
        };
        let resumen = resumen_fulfillment(&items);
        let evento_payload = json!({
            "order_id": order_id,
            "comprador_id": cliente_id,
            "cliente_parte_id": comando.cliente_parte_id,
            "items": items,
            "total_cents": total,
            "impuesto_cents": impuesto_total,
            "comision_cents": comision,
            "descuento_cupon_cents": descuento_cupon,
            "cupon_codigo": cupon_codigo,
            "moneda": moneda_orden,
            "estado": "creada",
            "tipo_orden": tipo_orden,
            "metodo_pago": metodo_pago,
            "moneda_pago": moneda_pago,
            "total_pagado_cents": total_pagado_cents,
            "fulfillment_config": resumen,
            "creado_en": creado_en,
        });
        let evento_outbox = json!({
            "event_id": Uuid::new_v4().to_string(),
            "tipo": ORDER_CREATED,
            "ocurrido_en": creado_en,
            "data": {
                "order_id": order_id,
                "comprador_id": cliente_id,
                "cliente_parte_id": comando.cliente_parte_id,
                "items": items,
                "total_cents": total,
                "impuesto_cents": impuesto_total,
                "moneda": moneda_orden,
                "moneda_pago": moneda_pago,
                "total_pagado_cents": total_pagado_cents,
                "estado": "creada",
                "tipo_orden": tipo_orden,
                "metodo_pago": metodo_pago,
                "fulfillment_config": resumen,
                "comision_cents": comision,
            },
        });

        let resultado = self
            .persistir_orden(
                &order_id,
                &evento_payload,
                &evento_outbox,
                &entregas,
                desde_carrito.then_some(cliente_id),
            )
            .await;
        if let Err(error) = resultado {
            if metodo_pago == "credito" {
                let _ = self.clientes.anular_credito(&order_id).await;
            }
            if let Some((empresa, codigo, _)) = &cupon_aplicado {
                self.clientes
                    .cupon_liberar(empresa, codigo, &order_id)
                    .await;
            }
            return Err(error);
        }
        let empresa_pago = empresa_unica(&items);
        let negocio_id = {
            let unicos: HashSet<&str> = items
                .iter()
                .filter_map(|item| item.negocio_id.as_deref())
                .collect();
            if unicos.len() == 1 {
                unicos.into_iter().next().map(str::to_string)
            } else {
                None
            }
        };
        let pago_url = self
            .pagos
            .registrar(
                &order_id,
                empresa_pago.as_deref(),
                negocio_id.as_deref(),
                &metodo,
                total_pagado_cents,
                &moneda_pago,
            )
            .await?;

        let primer_item = items.first();
        Ok(OrderView {
            id: order_id,
            vendedor_id: primer_item.and_then(|i| i.vendedor_id.clone()),
            comprador_id: cliente_id.to_string(),
            parte_id: comando.cliente_parte_id.clone(),
            tienda_id: primer_item.map(|i| i.tienda_id.clone()).unwrap_or_default(),
            negocio_id,
            estado: "creada".to_string(),
            total_cents: total,
            impuesto_cents: impuesto_total,
            comision_cents: comision,
            descuento_cupon_cents: descuento_cupon,
            cupon_codigo: cupon_codigo.clone(),
            moneda: moneda_orden,
            items,
            direccion_envio: Value::Null,
            creado_en: creado_en.clone(),
            actualizado_en: creado_en,
            tipo_orden: tipo_orden.to_string(),
            metodo_pago: metodo_pago.to_string(),
            pago_url,
            fulfillment_config: resumen,
            pagada_en: None,
            enviada_en: None,
            entregada_en: None,
            cancelada_en: None,
            devuelta_en: None,
        })
    }

    async fn persistir_orden(
        &self,
        order_id: &str,
        evento_payload: &Value,
        evento_outbox: &Value,
        entregas: &[EntregaPlan],
        vaciar_carrito: Option<&str>,
    ) -> Result<(), ApiError> {
        // El checkout lo inicia un comprador sin tenant. La proyección
        // `orders.orden_vista` y las entregas tienen `empresa_id`; con la
        // política RLS del comprador el INSERT falla. Se usa el claim de
        // servicio, igual que el registro de pagos.
        let mut tx = self.db.transaccion_servicio().await.map_err(error_db)?;
        OrderEventStore
            .append_en_transaccion(&mut tx, order_id, "OrderCreatedEvent", evento_payload)
            .await?;
        let sql = format!(
            "INSERT INTO {} (event_id, tipo, payload, estado, creado_en)
             VALUES ($1, $2, $3, 'pendiente', NOW())",
            self.outbox.tabla()
        );
        let event_id: String = evento_outbox
            .get("event_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        sqlx::query(&sql)
            .bind(parsear_uuid(&event_id)?)
            .bind(ORDER_CREATED)
            .bind(sqlx::types::Json(evento_outbox))
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        for entrega in entregas {
            sqlx::query(
                "INSERT INTO orders.entregas_digitales
                   (empresa_id, order_id, item_id, sku, comprador_id, cliente_parte_id,
                    token, recurso_url, max_descargas, expira_en)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                 ON CONFLICT (order_id, item_id) DO NOTHING",
            )
            .bind(parsear_uuid(&entrega.empresa_id)?)
            .bind(parsear_uuid(order_id)?)
            .bind(parsear_uuid(&entrega.item_id)?)
            .bind(&entrega.sku)
            .bind(parsear_uuid(&entrega.comprador_id)?)
            .bind(
                entrega
                    .cliente_parte_id
                    .as_deref()
                    .map(parsear_uuid)
                    .transpose()?,
            )
            .bind(parsear_uuid(&entrega.token)?)
            .bind(&entrega.recurso_url)
            .bind(entrega.max_descargas)
            .bind(entrega.expira_en)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }
        if let Some(comprador) = vaciar_carrito {
            self.carrito
                .vaciar_en_transaccion(&mut tx, comprador)
                .await?;
        }
        tx.commit().await.map_err(error_db)?;
        Ok(())
    }

    /// Cambio de estado (logística → orders) con evento + outbox atómicos.
    pub async fn transicionar(
        &self,
        order_id: &str,
        estado_nuevo: &str,
        motivo: Option<&str>,
        actor: Option<&str>,
        evento_adicional: Option<(&str, Value)>,
    ) -> Result<OrderView, ApiError> {
        let historia = OrderEventStore.historia_de(&self.db, order_id).await?;
        if historia.is_empty() {
            return Err(ApiError::no_encontrado("Orden", order_id));
        }
        let estado = reconstruir_desde_eventos(&historia)
            .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))?;
        self.tipos
            .validar(&estado.tipo_orden, &estado.estado, estado_nuevo)
            .await?;
        let actor_final = actor.unwrap_or("sistema");
        let ocurrido_en = iso_ms(Utc::now());
        let liberar_stock = estado_nuevo == "cancelada";
        let estaba_pagada = estado_pagado(&estado.estado);
        let motivo_cancelacion = motivo.unwrap_or("cancelacion");

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        OrderEventStore
            .append_en_transaccion(
                &mut tx,
                order_id,
                "OrderStatusUpdatedEvent",
                &json!({
                    "order_id": order_id,
                    "estado": estado_nuevo,
                    "previo_estado": estado.estado,
                    "motivo": motivo,
                    "actor": actor_final,
                    "ocurrido_en": ocurrido_en,
                }),
            )
            .await?;
        let evento = json!({
            "event_id": Uuid::new_v4().to_string(),
            "tipo": ORDER_STATUS_UPDATED,
            "ocurrido_en": ocurrido_en,
            "data": {
                "order_id": order_id,
                "estado": estado_nuevo,
                "previo_estado": estado.estado,
                "motivo": motivo,
                "actor": actor_final,
            },
        });
        self.insertar_outbox(&mut tx, &evento).await?;
        if liberar_stock {
            // Un pago manual que seguía pendiente ya no puede confirmarse: se
            // rechaza en la misma transacción que la cancelación. Los cargos
            // de una pasarela externa se dejan pendientes: el comprador pudo
            // pagar en el checkout alojado después del TTL y la conciliación
            // debe detectarlo y registrar un pago tardío.
            sqlx::query(
                "UPDATE orders.pagos AS p
                    SET estado = 'rechazado',
                        motivo_rechazo = COALESCE(NULLIF(motivo_rechazo, ''), $2),
                        actualizado_en = NOW()
                  WHERE p.order_id = $1::uuid
                    AND p.estado = 'pendiente'
                    AND NOT EXISTS (
                          SELECT 1
                            FROM orders.proveedores_pago pr
                           WHERE pr.codigo = p.proveedor
                             AND pr.tipo IN ('tilopay', 'pagadito')
                    )",
            )
            .bind(order_id)
            .bind(motivo_cancelacion)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;

            // Cualquier cancelación libera la reserva de stock. El evento es
            // exclusivo para inventario: no implica un asiento de devolución
            // si la orden nunca llegó a estar pagada.
            let cancelacion = json!({
                "event_id": Uuid::new_v4().to_string(),
                "tipo": ORDER_CANCELLED,
                "ocurrido_en": ocurrido_en,
                "data": {
                    "order_id": order_id,
                    "items": items_para_liberacion(&estado.items),
                    "motivo": motivo_cancelacion,
                },
            });
            self.insertar_outbox(&mut tx, &cancelacion).await?;

            // Si la orden ya estaba pagada, además corresponde revertir la
            // venta (finanzas y comisiones).
            if estaba_pagada {
                let devolucion = json!({
                    "event_id": Uuid::new_v4().to_string(),
                    "tipo": DEVOLUCION_SOLICITADA,
                    "ocurrido_en": ocurrido_en,
                    "data": {
                        "order_id": order_id,
                        "items": items_para_liberacion(&estado.items),
                        "motivo": motivo_cancelacion,
                    },
                });
                self.insertar_outbox(&mut tx, &devolucion).await?;
            }
        }
        if let Some((tipo, data)) = evento_adicional {
            // Evita duplicar los eventos que ya emite la cancelación.
            let ya_emitido = (tipo == ORDER_CANCELLED && liberar_stock)
                || (tipo == DEVOLUCION_SOLICITADA && liberar_stock && estaba_pagada);
            if !ya_emitido {
                let extra = json!({
                    "event_id": Uuid::new_v4().to_string(),
                    "tipo": tipo,
                    "ocurrido_en": ocurrido_en,
                    "data": data,
                });
                self.insertar_outbox(&mut tx, &extra).await?;
            }
        }
        tx.commit().await.map_err(error_db)?;

        let vista = self
            .vistas
            .encontrar(&self.db, order_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))?;
        if estado_nuevo == "pagada" {
            self.acumular_fidelizacion(&vista).await;
        }
        Ok(vista)
    }

    /// Acumula puntos de fidelización cuando la orden pasa a `pagada`.
    ///
    /// Es *best-effort*: un fallo de fidelización no revierte el pago ya
    /// confirmado. La acumulación es idempotente por `(tarjeta, orden)`, así
    /// que un reintento posterior no duplica puntos.
    async fn acumular_fidelizacion(&self, vista: &OrderView) {
        let Some(parte_id) = vista.parte_id.as_deref() else {
            return;
        };
        let mut por_empresa: HashMap<String, i64> = HashMap::new();
        for item in &vista.items {
            let subtotal = item.precio_unitario_cents.saturating_mul(item.cantidad);
            if subtotal > 0 {
                *por_empresa.entry(item.empresa_id.clone()).or_default() += subtotal;
            }
        }
        if por_empresa.is_empty() {
            return;
        }
        let empresas: Vec<String> = por_empresa.keys().cloned().collect();
        let Some(capacidades) = self
            .clientes
            .capacidades(&empresas, None, &["fidelizacion".to_string()])
            .await
        else {
            crate::logs::warn(
                "fidelizacion",
                crate::campos! { "msg": "identity no disponible; no se acumulan puntos", "order_id": vista.id },
            );
            return;
        };
        let activas: HashSet<String> = capacidades
            .iter()
            .filter(|fila| fila.activo && fila.capacidad_codigo == "fidelizacion")
            .map(|fila| fila.empresa_id.clone())
            .collect();
        let fidelizacion = Fidelizacion::nueva(Arc::clone(&self.db));
        for (empresa, subtotal) in por_empresa {
            if !activas.contains(&empresa) {
                continue;
            }
            if let Err(error) = fidelizacion
                .acumular_por_orden(&empresa, parte_id, &vista.id, subtotal)
                .await
            {
                crate::logs::error(
                    "fidelizacion",
                    crate::campos! { "msg": "no se pudieron acumular puntos", "order_id": vista.id, "empresa_id": empresa, "err": error.mensaje },
                );
            }
        }
    }

    async fn insertar_outbox(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        evento: &Value,
    ) -> Result<(), ApiError> {
        let sql = format!(
            "INSERT INTO {} (event_id, tipo, payload, estado, creado_en)
             VALUES ($1, $2, $3, 'pendiente', NOW())",
            self.outbox.tabla()
        );
        let event_id = evento
            .get("event_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let tipo = evento
            .get("tipo")
            .and_then(Value::as_str)
            .unwrap_or_default();
        sqlx::query(&sql)
            .bind(parsear_uuid(event_id)?)
            .bind(tipo)
            .bind(sqlx::types::Json(evento))
            .execute(&mut **tx)
            .await
            .map_err(error_db)?;
        Ok(())
    }

    pub async fn reproyectar(&self, order_id: &str) -> Result<Option<OrderView>, ApiError> {
        self.vistas.encontrar(&self.db, order_id).await
    }

    async fn resolver_precios(
        &self,
        lineas: &[LineaAgrupada],
        por_id: &HashMap<String, OfertaExterna>,
        comando: &ComandoCrear,
    ) -> Result<HashMap<String, PrecioResuelto>, ApiError> {
        let mut resultado = HashMap::new();
        let mut por_empresa: HashMap<String, Vec<Value>> = HashMap::new();
        for linea in lineas {
            let Some(oferta) = por_id.get(&linea.oferta_id) else {
                continue;
            };
            if oferta.item_type == "alquiler" {
                continue;
            }
            por_empresa
                .entry(oferta.empresa_id.clone())
                .or_default()
                .push(json!({
                    "clave": oferta.id,
                    "item_id": oferta.item_id,
                    "cantidad": linea.cantidad,
                }));
        }

        for (empresa_id, grupo) in por_empresa {
            let respuesta = self
                .clientes
                .precios(
                    &empresa_id,
                    grupo,
                    comando.lista_precio_codigo.as_deref(),
                    comando.segmento_codigo.as_deref(),
                )
                .await;
            match respuesta {
                PreciosExternos::NoEncontrado => {
                    if comando.lista_precio_codigo.is_some() || comando.segmento_codigo.is_some() {
                        let id = comando
                            .lista_precio_codigo
                            .as_deref()
                            .or(comando.segmento_codigo.as_deref())
                            .unwrap_or("");
                        return Err(ApiError::no_encontrado("Lista de precios", id));
                    }
                }
                PreciosExternos::Error => {
                    if comando.lista_precio_codigo.is_some() || comando.segmento_codigo.is_some() {
                        return Err(ApiError::servicio_no_disponible("pricing-service"));
                    }
                }
                PreciosExternos::Ok(precios) => {
                    for precio in precios {
                        resultado.insert(
                            precio.clave,
                            PrecioResuelto {
                                origen: precio.origen,
                                precio_unitario_cents: precio.precio_unitario_cents,
                                promocion: precio.promocion,
                            },
                        );
                    }
                }
            }
        }
        Ok(resultado)
    }

    async fn validar_capacidades(
        &self,
        items: &[ItemOrden],
        tipo_orden: &str,
        metodo_pago: &str,
    ) -> Result<HashMap<String, ConfiguracionOperativa>, ApiError> {
        let mut grupos: HashMap<(String, Option<String>), Vec<&ItemOrden>> = HashMap::new();
        for item in items {
            grupos
                .entry((item.empresa_id.clone(), item.negocio_id.clone()))
                .or_default()
                .push(item);
        }
        let empresas: Vec<String> = grupos
            .keys()
            .map(|(empresa, _)| empresa.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        for ((empresa, negocio), grupo) in grupos {
            let mut requeridas: HashSet<&str> = HashSet::new();
            if tipo_orden == "cotizacion" {
                requeridas.insert("cotizaciones");
            }
            if metodo_pago == "credito" {
                requeridas.insert("ventas_credito");
            }
            for item in &grupo {
                match item.fulfillment.as_deref() {
                    Some("delivery") => {
                        requeridas.insert("fulfillment_delivery");
                    }
                    Some("pickup") => {
                        requeridas.insert("fulfillment_pickup");
                    }
                    Some("onsite") => {
                        requeridas.insert("fulfillment_onsite");
                    }
                    Some("digital") => {
                        requeridas.insert("fulfillment_digital");
                    }
                    _ => {}
                }
            }
            if requeridas.is_empty() {
                continue;
            }
            let capacidades: Vec<String> = requeridas.iter().map(|c| c.to_string()).collect();
            let filas = self
                .clientes
                .capacidades(
                    std::slice::from_ref(&empresa),
                    negocio.as_deref(),
                    &capacidades,
                )
                .await
                .ok_or_else(|| ApiError::servicio_no_disponible("identity-service"))?;
            let activas: HashSet<&str> = filas
                .iter()
                .filter(|fila| fila.activo && fila.empresa_id == empresa)
                .map(|fila| fila.capacidad_codigo.as_str())
                .collect();
            let faltantes: Vec<String> = capacidades
                .iter()
                .filter(|capacidad| !activas.contains(capacidad.as_str()))
                .cloned()
                .collect();
            if !faltantes.is_empty() {
                return Err(ApiError::capacidad_no_activa(&faltantes, &empresa));
            }
        }
        let mut configuraciones = HashMap::new();
        for empresa in &empresas {
            let config = match self.clientes.configuracion(empresa).await {
                ConfiguracionExterna::Ok(config) => config,
                ConfiguracionExterna::SinConfiguracion => continue,
                ConfiguracionExterna::Error => {
                    return Err(ApiError::servicio_no_disponible("identity-service"))
                }
            };
            if !config.fulfillment.is_empty() {
                for item in items.iter().filter(|item| &item.empresa_id == empresa) {
                    let fulfillment = item.fulfillment.as_deref();
                    if !fulfillment_permitido(&config, fulfillment) {
                        return Err(ApiError::dominio(
                            "FULFILLMENT_NO_PERMITIDO",
                            format!(
                                "El modelo operativo de la empresa no permite fulfillment \"{}\".",
                                fulfillment.unwrap_or("none")
                            ),
                        ));
                    }
                }
            }
            configuraciones.insert(empresa.clone(), config);
        }
        Ok(configuraciones)
    }

    /// Aplica IVA por línea con la configuración fiscal efectiva del tenant.
    /// Devuelve `(total de la orden, impuesto total)`. El descuento de cupón se
    /// reparte proporcionalmente para que el desglose por línea sea coherente.
    async fn calcular_impuestos(
        &self,
        items: &mut [ItemOrden],
        descuento_cupon: i64,
        total_bruto: i64,
    ) -> Result<(i64, i64), ApiError> {
        let mut configs: HashMap<(String, Option<String>), Option<ConfiguracionFiscal>> =
            HashMap::new();
        for item in items.iter() {
            let clave = (item.empresa_id.clone(), item.negocio_id.clone());
            if let std::collections::hash_map::Entry::Vacant(entrada) = configs.entry(clave) {
                let config = self
                    .clientes
                    .configuracion_fiscal(&item.empresa_id, item.negocio_id.as_deref())
                    .await;
                entrada.insert(config);
            }
        }

        let ultimo = items.len().saturating_sub(1);
        let mut cupon_restante = descuento_cupon.max(0);
        let mut impuesto_total = 0i64;
        let mut total = 0i64;
        for (indice, item) in items.iter_mut().enumerate() {
            let neto = item
                .precio_unitario_cents
                .saturating_mul(item.cantidad)
                .saturating_sub(item.descuento_cents)
                .max(0);
            let cupon_linea = if total_bruto > 0 && cupon_restante > 0 {
                let calculado = if indice == ultimo {
                    cupon_restante
                } else {
                    ((descuento_cupon as f64 * neto as f64) / total_bruto as f64).round() as i64
                };
                calculado.clamp(0, neto.min(cupon_restante))
            } else {
                0
            };
            cupon_restante = (cupon_restante - cupon_linea).max(0);
            let neto_final = (neto - cupon_linea).max(0);
            let config = configs
                .get(&(item.empresa_id.clone(), item.negocio_id.clone()))
                .and_then(|config| config.as_ref());
            let activo = config
                .map(|config| config.impuesto_activo && config.tasa_iva > 0.0)
                .unwrap_or(false);
            let tasa = config.map(|config| config.tasa_iva).unwrap_or(0.0);
            let tasa_bps = (tasa * 10_000.0).round().max(0.0) as i64;
            let (impuesto, total_linea) = if activo {
                if config
                    .map(|config| config.precios_con_impuesto)
                    .unwrap_or(true)
                {
                    let base = if tasa > 0.0 {
                        ((neto_final as f64) / (1.0 + tasa)).round().max(0.0) as i64
                    } else {
                        neto_final
                    };
                    ((neto_final - base).max(0), neto_final)
                } else {
                    let impuesto = ((neto_final as f64) * tasa).round().max(0.0) as i64;
                    (impuesto, neto_final + impuesto)
                }
            } else {
                (0, neto_final)
            };
            item.impuesto_cents = impuesto;
            item.tasa_impuesto_bps = if activo { tasa_bps } else { 0 };
            item.total_linea_cents = total_linea;
            impuesto_total = impuesto_total.saturating_add(impuesto);
            total = total.saturating_add(total_linea);
        }
        Ok((total, impuesto_total))
    }

    async fn verificar_stock(&self, items: &[ItemOrden]) -> Result<(), ApiError> {
        let productos: Vec<&ItemOrden> = items
            .iter()
            .filter(|item| {
                item.item_type == "producto" && item.fulfillment.as_deref() != Some("digital")
            })
            .collect();
        if productos.is_empty() {
            return Ok(());
        }
        let skus: Vec<String> = {
            let mut vistos = HashSet::new();
            productos
                .iter()
                .filter(|item| vistos.insert(item.sku.to_lowercase()))
                .map(|item| item.sku.clone())
                .collect()
        };
        let Some(productos_catalogo) = self.clientes.items_lote(&skus).await else {
            return Ok(());
        };
        let por_clave: HashMap<String, &crate::clients::ItemStock> = productos_catalogo
            .iter()
            .map(|p| (format!("{}:{}", p.empresa_id, p.sku.to_lowercase()), p))
            .collect();
        let mut stock_por_ubicacion: HashMap<
            (String, String),
            Vec<crate::clients::StockUbicacion>,
        > = HashMap::new();
        for item in productos {
            let producto =
                por_clave.get(&format!("{}:{}", item.empresa_id, item.sku.to_lowercase()));
            let disponible = if let Some(ubicacion_id) = item.ubicacion_id.as_deref() {
                let clave = (item.empresa_id.clone(), ubicacion_id.to_string());
                if !stock_por_ubicacion.contains_key(&clave) {
                    let Some(stocks) = self
                        .clientes
                        .stock_ubicacion(
                            &item.empresa_id,
                            ubicacion_id,
                            std::slice::from_ref(&item.sku),
                        )
                        .await
                    else {
                        return Err(ApiError::servicio_no_disponible("inventory-service"));
                    };
                    stock_por_ubicacion.insert(clave.clone(), stocks);
                }
                let linea = stock_por_ubicacion.get(&clave).and_then(|lineas| {
                    lineas
                        .iter()
                        .find(|linea| linea.sku.eq_ignore_ascii_case(&item.sku))
                });
                let stock = linea.map(|linea| linea.stock).unwrap_or(0);
                let seriales = linea.map(|linea| linea.seriales_disponibles).unwrap_or(0);
                match producto {
                    Some(p) if p.track_serial => stock.min(seriales),
                    _ => stock,
                }
            } else {
                match producto {
                    Some(p) if p.track_serial => p.stock.min(p.seriales_disponibles),
                    Some(p) => p.stock,
                    None => 0,
                }
            };
            if producto.is_none() || disponible < item.cantidad {
                return Err(ApiError::nueva(
                    "CONFLICTO",
                    "Stock insuficiente (RN-03): se rechaza la orden.",
                    axum::http::StatusCode::CONFLICT,
                )
                .con_detalles(json!({
                    "sku": item.sku,
                    "cantidad_solicitada": item.cantidad,
                    "ubicacion_id": item.ubicacion_id,
                })));
            }
        }
        Ok(())
    }
}

fn lineas_de_carrito(items: &[ItemCarrito]) -> Vec<LineaEntrada> {
    items
        .iter()
        .map(|item| LineaEntrada {
            oferta_id: item.oferta_id.clone(),
            cantidad: item.cantidad,
            fulfillment: Some("none".to_string()),
            fulfillment_config: None,
            ubicacion_id: None,
        })
        .collect()
}

fn descuento_de_promocion(
    promocion: Option<&PromocionLinea>,
    precio_unitario: i64,
    cantidad: i64,
) -> Result<i64, ApiError> {
    let Some(promocion) = promocion else {
        return Ok(0);
    };
    let bruto = money::multiplicar(precio_unitario, cantidad);
    let descuento = match promocion.tipo.as_str() {
        "descuento_porcentaje" => {
            let porcentaje = promocion.descuento_porcentaje.unwrap_or(0).clamp(0, 100);
            ((bruto as f64 * porcentaje as f64) / 100.0).round() as i64
        }
        "2x1" => {
            let gratis = cantidad / promocion.cantidad_requerida.unwrap_or(2).max(1);
            gratis.saturating_mul(precio_unitario)
        }
        "descuento_cantidad" | "regalo" => {
            let requerida = promocion.cantidad_requerida.unwrap_or(1).max(1);
            let gratis = promocion.cantidad_gratis.unwrap_or(0).max(0);
            (cantidad / requerida)
                .saturating_mul(gratis)
                .min(cantidad)
                .saturating_mul(precio_unitario)
        }
        _ => 0,
    };
    Ok(descuento.clamp(0, bruto))
}

fn agrupar(lineas: &[LineaEntrada]) -> Result<Vec<LineaAgrupada>, ApiError> {
    let mut mapa: HashMap<String, LineaAgrupada> = HashMap::new();
    let mut orden: Vec<String> = Vec::new();
    for linea in lineas {
        let clave = linea.oferta_id.clone();
        let ubicacion = linea
            .ubicacion_id
            .as_deref()
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string);
        match mapa.get_mut(&clave) {
            Some(actual) => {
                if actual.fulfillment != linea.fulfillment {
                    return Err(ApiError::dominio(
                        "FULFILLMENT_CONFLICTO",
                        "Una misma oferta no puede tener dos fulfillments distintos en una orden.",
                    ));
                }
                if actual.ubicacion_id != ubicacion {
                    return Err(ApiError::dominio(
                        "UBICACION_CONFLICTO",
                        "Una misma oferta no puede despacharse desde dos ubicaciones en una orden.",
                    ));
                }
                actual.cantidad += linea.cantidad;
                if linea.fulfillment_config.is_some() {
                    actual.fulfillment_config = linea.fulfillment_config.clone();
                }
            }
            None => {
                orden.push(clave.clone());
                mapa.insert(
                    clave,
                    LineaAgrupada {
                        oferta_id: linea.oferta_id.clone(),
                        cantidad: linea.cantidad,
                        fulfillment: linea.fulfillment.clone(),
                        fulfillment_config: linea.fulfillment_config.clone(),
                        ubicacion_id: ubicacion,
                    },
                );
            }
        }
    }
    Ok(orden
        .into_iter()
        .filter_map(|clave| mapa.remove(&clave))
        .collect())
}

fn validar_fulfillment(
    valor: Option<&str>,
    tipo_orden: &str,
    item_type: &str,
    permitidos: &[String],
) -> Result<String, ApiError> {
    let fulfillment = valor.unwrap_or("none");
    if !permitidos.iter().any(|permitido| permitido == fulfillment) {
        return Err(ApiError::dominio(
            "FULFILLMENT_INVALIDO",
            "El fulfillment no es valido.",
        ));
    }
    if fulfillment == "delivery" && tipo_orden == "reserva" {
        return Err(ApiError::dominio(
            "FULFILLMENT_INVALIDO",
            "Una reserva no puede usar delivery.",
        ));
    }
    if tipo_orden == "reserva" && !["servicio", "alquiler"].contains(&item_type) {
        return Err(ApiError::dominio(
            "TIPO_ITEM_INVALIDO",
            "Las reservas aplican a servicios o alquileres.",
        ));
    }
    if tipo_orden == "reserva" && fulfillment != "onsite" {
        return Err(ApiError::dominio(
            "FULFILLMENT_INVALIDO",
            "Una reserva requiere fulfillment onsite.",
        ));
    }
    Ok(fulfillment.to_string())
}

fn texto_config(config: &Map<String, Value>, clave: &str) -> Option<String> {
    config
        .get(clave)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(ToString::to_string)
}

fn es_fecha(valor: &str) -> bool {
    regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").is_ok_and(|regex| regex.is_match(valor))
}

fn url_entrega_digital(atributos: Option<&Value>) -> Option<String> {
    let valor = atributos?.get("entrega_digital_url")?.as_str()?.trim();
    if !(valor.starts_with("http://") || valor.starts_with("https://") || valor.starts_with('/'))
        || valor.len() > 2048
    {
        return None;
    }
    Some(valor.to_string())
}

fn max_descargas_digital(atributos: Option<&Value>) -> Option<i64> {
    let valor = atributos?.get("entrega_digital_max_descargas")?;
    let numero = valor
        .as_i64()
        .or_else(|| valor.as_f64().map(|n| n as i64))?;
    (numero >= 1).then_some(numero)
}

fn expira_digital(atributos: Option<&Value>) -> Option<chrono::DateTime<Utc>> {
    let valor = atributos?.get("entrega_digital_expira_dias")?;
    let dias = valor
        .as_i64()
        .or_else(|| valor.as_f64().map(|n| n as i64))?;
    (dias >= 1).then(|| Utc::now() + Duration::days(dias))
}

pub fn resumen_fulfillment(items: &[ItemOrden]) -> Value {
    json!({
        "por_linea": items
            .iter()
            .map(|item| json!({
                "oferta_id": item.oferta_id,
                "fulfillment": item.fulfillment.clone().unwrap_or_else(|| "none".to_string()),
                "config": item.fulfillment_config.clone().unwrap_or_else(|| json!({})),
            }))
            .collect::<Vec<_>>(),
    })
}

pub fn reconstruir_desde_eventos(
    eventos: &[crate::event_store::EventoDeOrden],
) -> Option<EstadoDerivado> {
    let primero = eventos.first()?;
    if primero.tipo != "OrderCreatedEvent" {
        return None;
    }
    let inicial = primero.payload.as_object()?;
    let tipo_orden = inicial.get("tipo_orden")?.as_str()?.to_string();
    let mut estado = EstadoDerivado {
        order_id: inicial
            .get("order_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        comprador_id: inicial
            .get("comprador_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        items: inicial
            .get("items")
            .cloned()
            .and_then(|valor| serde_json::from_value(valor).ok())
            .unwrap_or_default(),
        total_cents: inicial
            .get("total_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        estado: inicial
            .get("estado")
            .and_then(Value::as_str)
            .unwrap_or("creada")
            .to_string(),
        motivo: None,
        creado_en: inicial
            .get("creado_en")
            .and_then(Value::as_str)
            .unwrap_or(&primero.creado_en)
            .to_string(),
        actualizado_en: primero.creado_en.clone(),
        tipo_orden,
    };
    for evento in eventos.iter().skip(1) {
        if evento.tipo == "OrderStatusUpdatedEvent" {
            let Some(datos) = evento.payload.as_object() else {
                continue;
            };
            if let Some(nuevo) = datos.get("estado").and_then(Value::as_str) {
                estado.estado = nuevo.to_string();
                estado.motivo = datos
                    .get("motivo")
                    .and_then(Value::as_str)
                    .map(ToString::to_string);
                estado.actualizado_en = evento.creado_en.clone();
            }
        }
    }
    Some(estado)
}

fn normalizar_moneda(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_uppercase();
    let valido = codigo.len() == 3 && codigo.chars().all(|c| c.is_ascii_uppercase());
    if !valido {
        return Err(ApiError::validacion(json!([
            {"campo": "moneda_pago", "restricciones": ["matches"]}
        ])));
    }
    Ok(codigo)
}

/// Estados en los que la venta ya se considera pagada y, por tanto, una
/// cancelación debe revertir finanzas y comisiones además de liberar stock.
fn estado_pagado(estado: &str) -> bool {
    matches!(
        estado,
        "pagada" | "en_preparacion" | "enviada" | "entregada"
    )
}

/// Líneas que comprometieron stock. La entrega digital no reserva inventario.
fn items_para_liberacion(items: &[ItemOrden]) -> Vec<Value> {
    items
        .iter()
        .filter(|item| item.fulfillment.as_deref() != Some("digital"))
        .map(|item| {
            json!({
                "empresa_id": &item.empresa_id,
                "sku": &item.sku,
                "cantidad": item.cantidad,
                "item_id": &item.item_id,
                "ubicacion_id": &item.ubicacion_id,
            })
        })
        .collect()
}

/// Empresa del pago cuando todas las líneas pertenecen a la misma; si no,
/// `None` (orden multi-empresa).
fn empresa_unica(items: &[ItemOrden]) -> Option<String> {
    let mut empresas = items.iter().map(|item| item.empresa_id.as_str());
    let primera = empresas.next()?;
    if empresas.all(|empresa| empresa == primera) {
        Some(primera.to_string())
    } else {
        None
    }
}

/// `true` si el fulfillment está permitido por el pack; lista vacía = sin
/// restricción (compatibilidad con packs antiguos).
fn fulfillment_permitido(
    config: &crate::clients::ConfiguracionOperativa,
    fulfillment: Option<&str>,
) -> bool {
    if config.fulfillment.is_empty() {
        return true;
    }
    match fulfillment {
        Some(valor) => config
            .fulfillment
            .iter()
            .any(|permitido| permitido == valor),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn promocion(tipo: &str) -> PromocionLinea {
        PromocionLinea {
            id: "p-1".into(),
            nombre: "Promo".into(),
            tipo: tipo.into(),
            cantidad_requerida: None,
            cantidad_gratis: None,
            descuento_porcentaje: None,
        }
    }

    #[test]
    fn debe_calcular_el_descuento_porcentual_y_el_2x1() {
        let mut porcentaje = promocion("descuento_porcentaje");
        porcentaje.descuento_porcentaje = Some(25);
        assert_eq!(
            descuento_de_promocion(Some(&porcentaje), 10_000, 2).expect("porcentaje"),
            5_000
        );

        let mut dos_por_uno = promocion("2x1");
        dos_por_uno.cantidad_requerida = Some(2);
        assert_eq!(
            descuento_de_promocion(Some(&dos_por_uno), 10_000, 4).expect("2x1"),
            20_000
        );
        assert_eq!(
            descuento_de_promocion(Some(&dos_por_uno), 10_000, 1).expect("sin 2x1"),
            0
        );
    }

    #[test]
    fn debe_tope_el_descuento_al_bruto() {
        let mut regalo = promocion("regalo");
        regalo.cantidad_requerida = Some(1);
        regalo.cantidad_gratis = Some(5);
        let descuento = descuento_de_promocion(Some(&regalo), 10_000, 2).expect("regalo");
        assert_eq!(descuento, 20_000);
    }

    #[test]
    fn debe_agrupar_lineas_de_la_misma_oferta() {
        let lineas = vec![
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 1,
                fulfillment: Some("none".into()),
                fulfillment_config: None,
                ubicacion_id: None,
            },
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 2,
                fulfillment: Some("none".into()),
                fulfillment_config: None,
                ubicacion_id: None,
            },
        ];
        let agrupadas = agrupar(&lineas).expect("agrupar");
        assert_eq!(agrupadas.len(), 1);
        assert_eq!(agrupadas[0].cantidad, 3);
    }

    #[test]
    fn debe_rechazar_fulfillments_distintos_para_la_misma_oferta() {
        let lineas = vec![
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 1,
                fulfillment: Some("none".into()),
                fulfillment_config: None,
                ubicacion_id: None,
            },
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 1,
                fulfillment: Some("digital".into()),
                fulfillment_config: None,
                ubicacion_id: None,
            },
        ];
        assert_eq!(
            agrupar(&lineas).expect_err("conflicto").codigo,
            "FULFILLMENT_CONFLICTO"
        );
    }

    #[test]
    fn debe_reconstruir_el_estado_desde_los_eventos() {
        let eventos = vec![
            crate::event_store::EventoDeOrden {
                event_id: "e-1".into(),
                aggregate_id: "o-1".into(),
                aggregate_type: "Order".into(),
                tipo: "OrderCreatedEvent".into(),
                payload: json!({
                    "order_id": "o-1",
                    "comprador_id": "c-1",
                    "items": [],
                    "total_cents": 100,
                    "estado": "creada",
                    "tipo_orden": "compra",
                    "creado_en": "2026-01-01T00:00:00.000Z"
                }),
                metadata: json!({}),
                version: 1,
                creado_en: "2026-01-01T00:00:00.000Z".into(),
            },
            crate::event_store::EventoDeOrden {
                event_id: "e-2".into(),
                aggregate_id: "o-1".into(),
                aggregate_type: "Order".into(),
                tipo: "OrderStatusUpdatedEvent".into(),
                payload: json!({"estado": "pagada", "motivo": "ok"}),
                metadata: json!({}),
                version: 2,
                creado_en: "2026-01-01T00:01:00.000Z".into(),
            },
        ];
        let estado = reconstruir_desde_eventos(&eventos).expect("estado");
        assert_eq!(estado.estado, "pagada");
        assert_eq!(estado.motivo.as_deref(), Some("ok"));
        assert_eq!(estado.total_cents, 100);
    }

    #[test]
    fn debe_ignorar_una_historia_sin_order_created() {
        let eventos = vec![crate::event_store::EventoDeOrden {
            event_id: "e-1".into(),
            aggregate_id: "o-1".into(),
            aggregate_type: "Order".into(),
            tipo: "OrderStatusUpdatedEvent".into(),
            payload: json!({}),
            metadata: json!({}),
            version: 1,
            creado_en: "2026-01-01T00:00:00.000Z".into(),
        }];
        assert!(reconstruir_desde_eventos(&eventos).is_none());
    }

    #[test]
    fn debe_permitir_fulfillment_sin_restriccion() {
        let config = crate::clients::ConfiguracionOperativa::default();
        assert!(fulfillment_permitido(&config, Some("delivery")));
        assert!(fulfillment_permitido(&config, None));
    }

    #[test]
    fn debe_respetar_los_fulfillment_del_modelo() {
        let config = crate::clients::ConfiguracionOperativa {
            fulfillment: vec!["delivery".to_string(), "pickup".to_string()],
            ..Default::default()
        };
        assert!(fulfillment_permitido(&config, Some("delivery")));
        assert!(fulfillment_permitido(&config, Some("pickup")));
        assert!(fulfillment_permitido(&config, None));
        assert!(!fulfillment_permitido(&config, Some("onsite")));
    }

    fn permitidos() -> Vec<String> {
        ["delivery", "pickup", "onsite", "digital", "none"]
            .iter()
            .map(|valor| (*valor).to_string())
            .collect()
    }

    #[test]
    fn debe_validar_las_reglas_de_fulfillment() {
        assert_eq!(
            validar_fulfillment(None, "compra", "producto", &permitidos()).expect("none"),
            "none"
        );
        assert_eq!(
            validar_fulfillment(Some("onsite"), "reserva", "servicio", &permitidos())
                .expect("onsite"),
            "onsite"
        );
        assert!(
            validar_fulfillment(Some("delivery"), "reserva", "servicio", &permitidos()).is_err()
        );
        assert!(validar_fulfillment(Some("onsite"), "reserva", "producto", &permitidos()).is_err());
        assert!(validar_fulfillment(Some("otro"), "compra", "producto", &permitidos()).is_err());
    }

    fn item(fulfillment: Option<&str>, ubicacion_id: Option<&str>) -> ItemOrden {
        ItemOrden {
            oferta_id: "o-1".into(),
            sku: "SKU-1".into(),
            producto_nombre: "Producto".into(),
            cantidad: 2,
            precio_unitario_cents: 10_000,
            descuento_cents: 0,
            impuesto_cents: 0,
            tasa_impuesto_bps: 0,
            total_linea_cents: 0,
            promocion_id: None,
            vendedor_id: None,
            tienda_id: "11111111-1111-1111-1111-111111111111".into(),
            empresa_id: "22222222-2222-2222-2222-222222222222".into(),
            negocio_id: None,
            participacion_porcentaje: None,
            comision: true,
            ubicacion_id: ubicacion_id.map(str::to_string),
            item_type: "producto".into(),
            item_id: "33333333-3333-3333-3333-333333333333".into(),
            fulfillment: fulfillment.map(str::to_string),
            fulfillment_config: None,
        }
    }

    #[test]
    fn debe_liberar_solo_las_lineas_que_comprometieron_stock() {
        let items = vec![
            item(
                Some("delivery"),
                Some("44444444-4444-4444-4444-444444444444"),
            ),
            item(Some("digital"), None),
        ];
        let liberables = items_para_liberacion(&items);
        assert_eq!(liberables.len(), 1);
        assert_eq!(liberables[0]["sku"], "SKU-1");
        assert_eq!(
            liberables[0]["ubicacion_id"],
            "44444444-4444-4444-4444-444444444444"
        );
    }

    #[test]
    fn debe_distinguir_una_orden_pagada_de_una_impaga() {
        for estado in ["pagada", "en_preparacion", "enviada", "entregada"] {
            assert!(estado_pagado(estado), "{estado}");
        }
        for estado in ["creada", "cancelada", "devuelta", "aceptada"] {
            assert!(!estado_pagado(estado), "{estado}");
        }
    }
}
