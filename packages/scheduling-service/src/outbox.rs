//! Outbox (ADR-03): el evento se persiste en la misma transacción del negocio.
//!
//! La publicación por RabbitMQ llega en el PR 5; aquí se conserva el contrato
//! de la fila (`event_id`, `tipo`, `payload`, `estado`, `creado_en`) y el
//! payload JSON del evento.

use crate::db::{jsonb, uuid_texto, Db};
use crate::errors::ApiError;
use crate::logs::contexto_actual;
use crate::metrics::{ahora_iso, Metrics};
use crate::rabbit::Rabbit;
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub const APPOINTMENT_CREATED: &str = "appointment.created";
pub const APPOINTMENT_CANCELLED: &str = "appointment.cancelled";
pub const APPOINTMENT_REMINDER_DUE: &str = "appointment.reminder.due";

pub const METRICAS_OUTBOX: [&str; 3] = [
    "core_engine_outbox_pending_events",
    "core_engine_outbox_published_events_total",
    "core_engine_outbox_publish_failures_total",
];

pub struct Outbox {
    tabla: String,
}

impl Outbox {
    /// La tabla viene de `OUTBOX_TABLA` (R-AR-3), nunca hardcodeada.
    pub fn nueva(tabla: impl Into<String>) -> Self {
        Self {
            tabla: tabla.into(),
        }
    }

    /// Reclama y publica hasta 50 eventos pendientes. La transacción mantiene
    /// el `FOR UPDATE SKIP LOCKED` mientras se confirma la publicación.
    pub async fn publicar_pendientes(
        &self,
        db: &Db,
        rabbit: &Rabbit,
        metrics: &Metrics,
    ) -> Result<(), ApiError> {
        let mut tx = db
            .transaccion()
            .await
            .map_err(crate::scheduling::error_db)?;

        let sql_conteo = format!(
            "SELECT COUNT(*)::bigint FROM {} WHERE estado = 'pendiente'",
            self.tabla
        );
        let pendientes: i64 = sqlx::query_scalar(&sql_conteo)
            .fetch_one(&mut *tx)
            .await
            .map_err(crate::scheduling::error_db)?;
        metrics.registrar_gauge(METRICAS_OUTBOX[0], pendientes as f64);

        let sql = format!(
            "SELECT event_id, payload FROM {}
             WHERE estado = 'pendiente'
             ORDER BY creado_en ASC LIMIT 50
             FOR UPDATE SKIP LOCKED",
            self.tabla
        );
        let filas = sqlx::query(&sql)
            .fetch_all(&mut *tx)
            .await
            .map_err(crate::scheduling::error_db)?;

        for fila in filas {
            let event_id = uuid_texto(&fila, "event_id").map_err(crate::scheduling::error_db)?;
            let payload = jsonb(&fila, "payload").map_err(crate::scheduling::error_db)?;
            if rabbit.publicar(&payload).await {
                let sql_update = format!(
                    "UPDATE {} SET estado = 'publicado', publicado_en = NOW()
                     WHERE event_id = $1 AND estado = 'pendiente'",
                    self.tabla
                );
                sqlx::query(&sql_update)
                    .bind(crate::scheduling::parsear_uuid(&event_id)?)
                    .execute(&mut *tx)
                    .await
                    .map_err(crate::scheduling::error_db)?;
                metrics.incrementar_contador(METRICAS_OUTBOX[1]);
            } else {
                metrics.incrementar_contador(METRICAS_OUTBOX[2]);
            }
        }

        tx.commit().await.map_err(crate::scheduling::error_db)?;
        Ok(())
    }

    pub async fn insertar_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        tipo: &str,
        data: Value,
    ) -> Result<String, ApiError> {
        let event_id = Uuid::new_v4();
        let mut evento = Map::new();
        evento.insert("event_id".into(), json!(event_id.to_string()));
        evento.insert("tipo".into(), json!(tipo));
        evento.insert("ocurrido_en".into(), json!(ahora_iso()));
        if let Some(ctx) = contexto_actual() {
            if !ctx.request_id.is_empty() {
                evento.insert("request_id".into(), json!(ctx.request_id));
            }
        }
        evento.insert("data".into(), data);
        let payload = Value::Object(evento);

        let sql = format!(
            "INSERT INTO {} (event_id, tipo, payload, estado, creado_en)
             VALUES ($1, $2, $3, 'pendiente', NOW())",
            self.tabla
        );
        sqlx::query(&sql)
            .bind(event_id)
            .bind(tipo)
            .bind(sqlx::types::Json(&payload))
            .execute(&mut **tx)
            .await
            .map_err(crate::scheduling::error_db)?;
        Ok(event_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_crear_la_tabla_desde_la_configuracion() {
        let outbox = Outbox::nueva("scheduling.outbox");
        assert_eq!(outbox.tabla, "scheduling.outbox");
    }

    #[test]
    fn debe_nombrar_los_eventos_igual_que_el_contrato() {
        assert_eq!(APPOINTMENT_CREATED, "appointment.created");
        assert_eq!(APPOINTMENT_CANCELLED, "appointment.cancelled");
        assert_eq!(APPOINTMENT_REMINDER_DUE, "appointment.reminder.due");
    }
}
