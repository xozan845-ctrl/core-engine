//! Consumidor de la cola `notifications.todos` (Tabla 22).
//!
//! Reutiliza los eventos ya publicados por la saga; no crea eventos nuevos.
//! Idempotencia por `event_id`: si el envío falla, el mensaje se reintenta y el
//! evento no queda marcado; si ya se procesó, se descarta.

use crate::campos;
use crate::db::Db;
use crate::errors::ErrorNotificacion;
use crate::logs;
use crate::notificaciones::NotificacionesPort;
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub struct ConsumidorNotificaciones {
    db: Arc<Db>,
    notificaciones: Arc<dyn NotificacionesPort>,
}

impl ConsumidorNotificaciones {
    pub fn nuevo(db: Arc<Db>, notificaciones: Arc<dyn NotificacionesPort>) -> Self {
        Self { db, notificaciones }
    }

    pub async fn manejar(&self, evento: Value) -> Result<(), ErrorNotificacion> {
        let event_id = parsear_event_id(&evento)?;
        if self.ya_procesado(event_id).await? {
            return Ok(());
        }
        self.notificaciones.procesar(&evento).await?;
        self.marcar_procesado(event_id, &evento).await
    }

    async fn ya_procesado(&self, event_id: Uuid) -> Result<bool, ErrorNotificacion> {
        self.db.evento_procesado(event_id).await.map_err(Into::into)
    }

    async fn marcar_procesado(
        &self,
        event_id: Uuid,
        evento: &Value,
    ) -> Result<(), ErrorNotificacion> {
        let tipo = evento.get("tipo").and_then(Value::as_str).unwrap_or("");
        self.db
            .marcar_evento(event_id, tipo)
            .await
            .map_err(Into::into)
    }
}

/// Handler para `Rabbit::consumir`: envuelve el mensaje en el contexto de
/// correlación del servicio (igual que `ejecutarConContexto`).
pub fn handler_consumidor(consumidor: Arc<ConsumidorNotificaciones>) -> crate::rabbit::Handler {
    Arc::new(move |evento| {
        let consumidor = Arc::clone(&consumidor);
        Box::pin(async move {
            let request_id = evento
                .get("event_id")
                .and_then(Value::as_str)
                .map(|id| format!("notifications-event-{id}"))
                .unwrap_or_else(|| "notifications-event-desconocido".to_string());
            crate::logs::ejecutar_con_contexto(contexto(request_id), async move {
                consumidor
                    .manejar(evento)
                    .await
                    .map_err(|error| error.mensaje())
            })
            .await
        })
    })
}

fn parsear_event_id(evento: &Value) -> Result<Uuid, ErrorNotificacion> {
    let event_id = evento
        .get("event_id")
        .and_then(Value::as_str)
        .ok_or_else(|| ErrorNotificacion::Evento("El evento no tiene event_id.".to_string()))?;
    Uuid::parse_str(event_id)
        .map_err(|_| ErrorNotificacion::Evento("El event_id no es un UUID valido.".to_string()))
}

fn contexto(request_id: String) -> crate::logs::ContextoCorrelacion {
    crate::logs::ContextoCorrelacion {
        request_id,
        servicio: "notifications-consumer".to_string(),
        usuario_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
        rol: Some("super_admin".to_string()),
        tenant_id: None,
    }
}

/// Log informativo del arranque del consumidor.
pub fn anunciar_consumidor() {
    logs::info(
        "notifications.consumer",
        campos! { "msg": "Consumidor de notificaciones activo" },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;
    use std::sync::Mutex;

    const EVENT_ID: &str = "33333333-3333-3333-3333-333333333333";

    fn evento() -> Value {
        json!({
            "event_id": EVENT_ID,
            "tipo": "payment.procesado",
            "ocurrido_en": "2026-11-11T00:00:00.000Z",
            "data": { "order_id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee" },
        })
    }

    struct ProcesadorFalso {
        veces: Mutex<u32>,
        falla: bool,
    }

    #[async_trait]
    impl NotificacionesPort for ProcesadorFalso {
        async fn procesar(&self, _evento: &Value) -> Result<(), ErrorNotificacion> {
            *self.veces.lock().expect("lock") += 1;
            if self.falla {
                return Err(ErrorNotificacion::Correo("SMTP caído".to_string()));
            }
            Ok(())
        }
    }

    #[test]
    fn debe_construir_el_contexto_del_consumidor() {
        let ctx = contexto(format!("notifications-event-{EVENT_ID}"));
        assert_eq!(ctx.servicio, "notifications-consumer");
        assert_eq!(ctx.rol.as_deref(), Some("super_admin"));
        assert_eq!(
            ctx.usuario_id.as_deref(),
            Some("00000000-0000-0000-0000-000000000000")
        );
    }

    #[test]
    fn debe_rechazar_eventos_sin_event_id_valido() {
        let error =
            parsear_event_id(&json!({ "tipo": "payment.procesado" })).expect_err("sin event_id");
        assert_eq!(error.mensaje(), "El evento no tiene event_id.");

        let error =
            parsear_event_id(&json!({ "event_id": "no-es-uuid" })).expect_err("uuid invalido");
        assert_eq!(error.mensaje(), "El event_id no es un UUID valido.");

        assert_eq!(
            parsear_event_id(&evento()).expect("uuid").to_string(),
            EVENT_ID
        );
    }

    #[test]
    fn debe_anunciar_el_consumidor_sin_entrar_en_panico() {
        anunciar_consumidor();
    }
}
