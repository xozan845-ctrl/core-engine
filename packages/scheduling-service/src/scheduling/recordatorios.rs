use super::helpers::*;
use super::*;

impl Agenda {
    pub async fn listar_recordatorios(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT id, cita_id, tipo, programado_en, estado, enviado_en
             FROM scheduling.recordatorios
             WHERE empresa_id = $1 ORDER BY programado_en ASC LIMIT 500",
        )
        .bind(empresa)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_recordatorio).collect()
    }

    /// Materializa una reserva de `order.created` (idempotente por `order_id`).
    pub async fn crear_desde_reserva(&self, orden: &OrdenReserva) -> Result<Value, ApiError> {
        let order_id = parsear_uuid(&orden.order_id)?;
        // Consumidor de eventos: contexto de servicio (super_admin) para que
        // la RLS de citas no oculte la reserva ya creada.
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let existente = sqlx::query(
            "SELECT id, empresa_id, recurso_id, parte_id, order_id, item_id, estado,
                    inicia_en, termina_en, notas, metadata, creado_en, actualizado_en
             FROM scheduling.citas WHERE order_id = $1",
        )
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        if let Some(fila) = existente {
            return fila_cita(&fila);
        }

        let item = orden.items.iter().find(|item| {
            matches!(item.item_type.as_str(), "servicio" | "alquiler")
                && item.fulfillment.as_deref() == Some("onsite")
        });
        let Some(item) = item else {
            return Err(configuracion_requerida());
        };
        let config = item
            .fulfillment_config
            .clone()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        let (Some(recurso_id), Some(inicia_en), Some(termina_en)) = (
            texto_config(&config, "recurso_id"),
            texto_config(&config, "inicia_en"),
            texto_config(&config, "termina_en"),
        ) else {
            return Err(configuracion_requerida());
        };

        let mut metadata = json!({ "origen": "order.created" });
        if let Some(reserva) = config.get("metadata").filter(|valor| valor.is_object()) {
            metadata["reserva"] = reserva.clone();
        }

        self.crear_cita(
            &item.empresa_id,
            &CrearCita {
                recurso_id,
                parte_id: orden.cliente_parte_id.clone(),
                order_id: Some(orden.order_id.clone()),
                item_id: item.item_id.clone(),
                inicia_en,
                termina_en,
                notas: texto_config(&config, "notas"),
                metadata,
            },
        )
        .await
    }

    /// Recordatorios vencidos: los marca enviados y publica el evento (PR 5).
    pub async fn procesar_recordatorios(&self) -> Result<usize, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT r.id, r.cita_id, r.empresa_id, r.tipo, r.programado_en
             FROM scheduling.recordatorios r
             WHERE r.estado = 'pendiente' AND r.programado_en <= NOW()
             ORDER BY r.programado_en ASC LIMIT 100 FOR UPDATE SKIP LOCKED",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;

        for fila in &filas {
            let recordatorio_id = uuid_texto(fila, "id").map_err(error_db)?;
            sqlx::query(
                "UPDATE scheduling.recordatorios
                    SET estado = 'enviado', enviado_en = NOW()
                  WHERE id = $1 AND estado = 'pendiente'",
            )
            .bind(parsear_uuid(&recordatorio_id)?)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;

            let data = json!({
                "recordatorio_id": recordatorio_id,
                "cita_id": uuid_texto(fila, "cita_id").map_err(error_db)?,
                "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
                "tipo": fila.try_get::<String, _>("tipo").map_err(error_db)?,
                "programado_en": timestamp(fila, "programado_en").map_err(error_db)?,
            });
            self.outbox
                .insertar_en_transaccion(&mut tx, APPOINTMENT_REMINDER_DUE, data)
                .await?;
        }

        let total = filas.len();
        tx.commit().await.map_err(error_db)?;
        Ok(total)
    }
}
