use super::comunes::*;
use super::prelude::*;
use super::*;

impl Checkout {
    /// Cambio de estado (logística → orders) con evento + outbox atómicos.
    pub async fn transicionar(
        &self,
        order_id: &str,
        estado_nuevo: &str,
        motivo: Option<&str>,
        actor: Option<&str>,
        evento_adicional: Option<(&str, Value)>,
    ) -> Result<OrderView, ApiError> {
        // Serializa todos los cambios de estado de esta orden. Sin el lock, el
        // barrido de expiración y la confirmación de pago podrían reconstruir
        // ambos `creada`, y el segundo evento podría cancelar una orden ya pagada.
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(order_id)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;

        // Leer la historia solo después de tomar el lock; así se valida contra
        // el estado que dejó la transición ganadora.
        let historia = OrderEventStore.historia_de(&self.db, order_id).await?;
        if historia.is_empty() {
            return Err(ApiError::no_encontrado("Orden", order_id));
        }
        let estado = reconstruir_desde_eventos(&historia)
            .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))?;
        if motivo == Some("expirada_por_ttl") && estado_pagado(&estado.estado) {
            return Err(ApiError::dominio(
                "TRANSICION_INVALIDA",
                "La orden recibió el pago antes de vencer; el barrido no puede cancelarla.",
            ));
        }
        self.tipos
            .validar(&estado.tipo_orden, &estado.estado, estado_nuevo)
            .await?;
        let actor_final = actor.unwrap_or("sistema");
        let ocurrido_en = iso_ms(Utc::now());
        let liberar_stock = estado_nuevo == "cancelada";
        let estaba_pagada = estado_pagado(&estado.estado);
        let motivo_cancelacion = motivo.unwrap_or("cancelacion");
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
                "empresa_ids": empresas_de_items(&estado.items),
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
                    AND p.proveedor NOT IN ('tilopay', 'pagadito')
                    AND p.proveedor <> $3
                    AND NOT EXISTS (
                          SELECT 1
                            FROM orders.proveedores_pago pr
                           WHERE pr.codigo = p.proveedor
                             AND pr.tipo IN ('tilopay', 'pagadito')
                    )",
            )
            .bind(order_id)
            .bind(motivo_cancelacion)
            .bind(self.pagos.proveedor_default_codigo())
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
    pub(super) async fn acumular_fidelizacion(&self, vista: &OrderView) {
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

    pub(super) async fn insertar_outbox(
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
}
