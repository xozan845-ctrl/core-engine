use super::comunes::*;
use super::prelude::*;
use super::*;

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
            let impuesto_codigo = linea.impuesto_codigo.clone().or_else(|| {
                oferta
                    .atributos
                    .as_ref()
                    .and_then(|atributos| atributos.get("impuesto_codigo"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
            items.push(ItemOrden {
                oferta_id: oferta.id.clone(),
                sku: oferta.sku.clone(),
                producto_nombre: oferta.producto_nombre.clone(),
                cantidad: linea.cantidad,
                precio_unitario_cents: precio_unitario,
                descuento_cents: descuento,
                impuesto_cents: 0,
                tasa_impuesto_bps: 0,
                impuesto_codigo,
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
}
