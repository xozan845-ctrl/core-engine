//! `CreateOrderCommandHandler`: valida las reglas (RN-01/03/04/05), persiste el
//! evento y el outbox en la misma transacción y devuelve la vista de la orden.
//! Además aplica transiciones (logística → orders) y replay.

use crate::carrito::{Carrito, ItemCarrito};
use crate::clients::{ClientesPort, CreditoEstado, OfertaExterna, PreciosExternos};
use crate::db::{iso_ms, Db};
use crate::errors::ApiError;
use crate::event_store::{error_db, parsear_uuid, OrderEventStore};
use crate::money;
use crate::order_state::{validar_transicion_por_tipo, ItemOrden, FULFILLMENTS, TIPOS_ORDEN};
use crate::outbox::Outbox;
use crate::pedidos::Pedidos;
use crate::views::{OrderView, OrderViewRepository};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use uuid::Uuid;

pub const ORDER_CREATED: &str = "order.created";
pub const ORDER_STATUS_UPDATED: &str = "order.status.updated";
pub const PAYMENT_PROCESADO: &str = "payment.procesado";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineaEntrada {
    pub oferta_id: String,
    pub cantidad: i64,
    #[serde(default)]
    pub fulfillment: Option<String>,
    #[serde(default)]
    pub fulfillment_config: Option<Value>,
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
    pub lista_precio_codigo: Option<String>,
    #[serde(default)]
    pub segmento_codigo: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct LineaAgrupada {
    oferta_id: String,
    cantidad: i64,
    fulfillment: Option<String>,
    fulfillment_config: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
struct PrecioResuelto {
    origen: String,
    precio_unitario_cents: i64,
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
    pub tasa_comision: f64,
}

impl Checkout {
    #[allow(clippy::too_many_arguments)]
    pub async fn ejecutar(
        &self,
        comando: &ComandoCrear,
        cliente_id: &str,
    ) -> Result<OrderView, ApiError> {
        let tipo_orden = validar_tipo_orden(&comando.tipo_orden)?;
        let es_cotizacion = tipo_orden == "cotizacion";
        let es_reserva = tipo_orden == "reserva";
        let metodo_pago = if comando.metodo_pago.as_deref() == Some("credito") {
            "credito"
        } else {
            "simulado"
        };
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
            let fulfillment =
                validar_fulfillment(linea.fulfillment.as_deref(), tipo_orden, &oferta.item_type)?;
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
            total += money::multiplicar(precio_unitario, linea.cantidad);
            items.push(ItemOrden {
                oferta_id: oferta.id.clone(),
                sku: oferta.sku.clone(),
                producto_nombre: oferta.producto_nombre.clone(),
                cantidad: linea.cantidad,
                precio_unitario_cents: precio_unitario,
                vendedor_id: oferta.vendedor_id.clone(),
                tienda_id: oferta.tienda_id.clone(),
                empresa_id: oferta.empresa_id.clone(),
                participacion_porcentaje: oferta.participacion_porcentaje,
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

        self.validar_capacidades(&items, tipo_orden, metodo_pago)
            .await?;

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
                    .ok_or_else(|| ApiError::servicio_no_disponible("catalog-service"))?;
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
                total += money::multiplicar(cotizacion.total_cents, item.cantidad);
            }
        }

        if !es_cotizacion {
            self.verificar_stock(&items).await?;
        }

        let order_id = Uuid::new_v4().to_string();
        let creado_en = iso_ms(Utc::now());

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
                    items.first().map(|i| i.vendedor_id.as_str()),
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

        let resumen = resumen_fulfillment(&items);
        let evento_payload = json!({
            "order_id": order_id,
            "comprador_id": cliente_id,
            "cliente_parte_id": comando.cliente_parte_id,
            "items": items,
            "total_cents": total,
            "estado": "creada",
            "tipo_orden": tipo_orden,
            "metodo_pago": metodo_pago,
            "fulfillment_config": resumen,
            "creado_en": creado_en,
        });

        let comision = if es_cotizacion {
            0
        } else {
            money::comision(total, self.tasa_comision)?
        };
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
                "moneda": moneda_orden.clone().unwrap_or_else(|| "NIO".to_string()),
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
            return Err(error);
        }

        let primer_item = items.first();
        Ok(OrderView {
            id: order_id,
            vendedor_id: primer_item
                .map(|i| i.vendedor_id.clone())
                .unwrap_or_default(),
            comprador_id: cliente_id.to_string(),
            parte_id: comando.cliente_parte_id.clone(),
            tienda_id: primer_item.map(|i| i.tienda_id.clone()).unwrap_or_default(),
            estado: "creada".to_string(),
            total_cents: total,
            comision_cents: if es_cotizacion {
                0
            } else {
                money::comision(total, 0.12)?
            },
            moneda: moneda_orden.unwrap_or_else(|| "NIO".to_string()),
            items,
            direccion_envio: Value::Null,
            creado_en: creado_en.clone(),
            actualizado_en: creado_en,
            tipo_orden: tipo_orden.to_string(),
            metodo_pago: metodo_pago.to_string(),
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
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
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
        validar_transicion_por_tipo(&estado.tipo_orden, &estado.estado, estado_nuevo)?;
        let actor_final = actor.unwrap_or("sistema");
        let ocurrido_en = iso_ms(Utc::now());

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
        if let Some((tipo, data)) = evento_adicional {
            let extra = json!({
                "event_id": Uuid::new_v4().to_string(),
                "tipo": tipo,
                "ocurrido_en": ocurrido_en,
                "data": data,
            });
            self.insertar_outbox(&mut tx, &extra).await?;
        }
        tx.commit().await.map_err(error_db)?;

        self.vistas
            .encontrar(&self.db, order_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))
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
                        return Err(ApiError::servicio_no_disponible("catalog-service"));
                    }
                }
                PreciosExternos::Ok(precios) => {
                    for precio in precios {
                        resultado.insert(
                            precio.clave,
                            PrecioResuelto {
                                origen: precio.origen,
                                precio_unitario_cents: precio.precio_unitario_cents,
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
    ) -> Result<(), ApiError> {
        let mut requeridas: HashSet<&str> = HashSet::new();
        if tipo_orden == "cotizacion" {
            requeridas.insert("cotizaciones");
        }
        if metodo_pago == "credito" {
            requeridas.insert("ventas_credito");
        }
        for item in items {
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
            return Ok(());
        }
        let empresas: Vec<String> = {
            let mut vistos = HashSet::new();
            items
                .iter()
                .filter(|item| vistos.insert(item.empresa_id.clone()))
                .map(|item| item.empresa_id.clone())
                .collect()
        };
        let capacidades: Vec<String> = requeridas.iter().map(|c| c.to_string()).collect();
        let filas = self
            .clientes
            .capacidades(&empresas, &capacidades)
            .await
            .ok_or_else(|| ApiError::servicio_no_disponible("identity-service"))?;
        let activas: HashSet<String> = filas
            .iter()
            .filter(|fila| fila.activo)
            .map(|fila| format!("{}:{}", fila.empresa_id, fila.capacidad_codigo))
            .collect();
        for empresa in empresas {
            let faltantes: Vec<String> = capacidades
                .iter()
                .filter(|capacidad| !activas.contains(&format!("{empresa}:{capacidad}")))
                .cloned()
                .collect();
            if !faltantes.is_empty() {
                return Err(ApiError::capacidad_no_activa(&faltantes, &empresa));
            }
        }
        Ok(())
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
        for item in productos {
            let producto =
                por_clave.get(&format!("{}:{}", item.empresa_id, item.sku.to_lowercase()));
            let disponible = match producto {
                Some(p) if p.track_serial => p.stock.min(p.seriales_disponibles),
                Some(p) => p.stock,
                None => 0,
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
        })
        .collect()
}

fn agrupar(lineas: &[LineaEntrada]) -> Result<Vec<LineaAgrupada>, ApiError> {
    let mut mapa: HashMap<String, LineaAgrupada> = HashMap::new();
    let mut orden: Vec<String> = Vec::new();
    for linea in lineas {
        let clave = linea.oferta_id.clone();
        match mapa.get_mut(&clave) {
            Some(actual) => {
                if actual.fulfillment != linea.fulfillment {
                    return Err(ApiError::dominio(
                        "FULFILLMENT_CONFLICTO",
                        "Una misma oferta no puede tener dos fulfillments distintos en una orden.",
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

fn validar_tipo_orden(valor: &str) -> Result<&str, ApiError> {
    if valor.is_empty() {
        return Err(ApiError::dominio(
            "TIPO_ORDEN_REQUERIDO",
            "El tipo de orden es obligatorio.",
        ));
    }
    if !TIPOS_ORDEN.contains(&valor) {
        return Err(ApiError::dominio(
            "TIPO_ORDEN_INVALIDO",
            "El tipo de orden no es valido.",
        ));
    }
    Ok(valor)
}

fn validar_fulfillment(
    valor: Option<&str>,
    tipo_orden: &str,
    item_type: &str,
) -> Result<String, ApiError> {
    let fulfillment = valor.unwrap_or("none");
    if !FULFILLMENTS.contains(&fulfillment) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_agrupar_lineas_de_la_misma_oferta() {
        let lineas = vec![
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 1,
                fulfillment: Some("none".into()),
                fulfillment_config: None,
            },
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 2,
                fulfillment: Some("none".into()),
                fulfillment_config: None,
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
            },
            LineaEntrada {
                oferta_id: "o-1".into(),
                cantidad: 1,
                fulfillment: Some("digital".into()),
                fulfillment_config: None,
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
    fn debe_validar_las_reglas_de_fulfillment() {
        assert_eq!(
            validar_fulfillment(None, "compra", "producto").expect("none"),
            "none"
        );
        assert_eq!(
            validar_fulfillment(Some("onsite"), "reserva", "servicio").expect("onsite"),
            "onsite"
        );
        assert!(validar_fulfillment(Some("delivery"), "reserva", "servicio").is_err());
        assert!(validar_fulfillment(Some("onsite"), "reserva", "producto").is_err());
        assert!(validar_fulfillment(Some("otro"), "compra", "producto").is_err());
    }
}
