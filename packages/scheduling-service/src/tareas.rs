//! Tareas de fondo: consumidor de `order.created`, publicador del Outbox y
//! worker de recordatorios.

use crate::db::Db;
use crate::errors::ApiError;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::metrics::Metrics;
use crate::outbox::Outbox;
use crate::rabbit::Rabbit;
use crate::scheduling::{AgendaPort, OrdenReserva};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

const USUARIO_SERVICIO: &str = "00000000-0000-0000-0000-000000000000";

fn contexto(request_id: String, servicio: &str) -> ContextoCorrelacion {
    ContextoCorrelacion {
        request_id,
        servicio: servicio.to_string(),
        usuario_id: Some(USUARIO_SERVICIO.to_string()),
        rol: Some("super_admin".to_string()),
        tenant_id: None,
    }
}

/// Consumidor de `order.created` (cola `scheduling.orders`), idempotente por
/// `event_id` igual que el consumidor TypeScript.
pub struct SchedulingConsumer {
    db: Arc<Db>,
    agenda: Arc<dyn AgendaPort>,
}

impl SchedulingConsumer {
    pub fn nuevo(db: Arc<Db>, agenda: Arc<dyn AgendaPort>) -> Self {
        Self { db, agenda }
    }

    pub async fn manejar(&self, evento: Value) -> Result<(), ApiError> {
        let event_id = evento
            .get("event_id")
            .and_then(Value::as_str)
            .ok_or_else(|| ApiError::dominio("EVENTO_INVALIDO", "El evento no tiene event_id."))?
            .to_string();
        let tipo = evento.get("tipo").and_then(Value::as_str).unwrap_or("");
        if tipo != "order.created" {
            return Ok(());
        }
        if self.ya_procesado(&event_id).await? {
            return Ok(());
        }

        let data = evento.get("data").cloned().unwrap_or(Value::Null);
        if data.get("tipo_orden").and_then(Value::as_str) == Some("reserva") {
            let cliente = data
                .get("cliente_parte_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|valor| !valor.is_empty());
            if cliente.is_none() {
                return Err(ApiError::dominio(
                    "PARTE_REQUERIDA",
                    "Una reserva requiere cliente_parte_id.",
                ));
            }
            let orden: OrdenReserva = serde_json::from_value(data).map_err(|error| {
                ApiError::dominio("EVENTO_INVALIDO", format!("Evento invalido: {error}"))
            })?;
            self.agenda.crear_desde_reserva(&orden).await?;
        }

        self.marcar_procesado(&event_id, tipo).await
    }

    async fn ya_procesado(&self, event_id: &str) -> Result<bool, ApiError> {
        let id = crate::scheduling::parsear_uuid(event_id)?;
        let fila = sqlx::query("SELECT 1 FROM scheduling.eventos_procesados WHERE event_id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(crate::scheduling::error_db)?;
        Ok(fila.is_some())
    }

    async fn marcar_procesado(&self, event_id: &str, tipo: &str) -> Result<(), ApiError> {
        let id = crate::scheduling::parsear_uuid(event_id)?;
        sqlx::query(
            "INSERT INTO scheduling.eventos_procesados (event_id, tipo, procesado_en)
             VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(id)
        .bind(tipo)
        .execute(self.db.pool())
        .await
        .map_err(crate::scheduling::error_db)?;
        Ok(())
    }
}

/// Handler para `Rabbit::consumir`: envuelve el mensaje en el contexto de
/// correlación del servicio (igual que `ejecutarConContexto`).
pub fn handler_consumidor(consumidor: Arc<SchedulingConsumer>) -> crate::rabbit::Handler {
    Arc::new(move |evento| {
        let consumidor = Arc::clone(&consumidor);
        Box::pin(async move {
            let request_id = evento
                .get("event_id")
                .and_then(Value::as_str)
                .map(|id| format!("scheduling-event-{id}"))
                .unwrap_or_else(|| "scheduling-event-desconocido".to_string());
            ejecutar_con_contexto(contexto(request_id, "scheduling-consumer"), async move {
                consumidor
                    .manejar(evento)
                    .await
                    .map_err(|error| error.mensaje)
            })
            .await
        })
    })
}

/// Publica el Outbox cada segundo (R-AR-3).
pub fn iniciar_outbox(
    outbox: Outbox,
    db: Arc<Db>,
    rabbit: Arc<Rabbit>,
    metrics: Arc<Metrics>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.tick().await; // setInterval: el primer ciclo es al segundo
        loop {
            ticker.tick().await;
            if let Err(error) = outbox.publicar_pendientes(&db, &rabbit, &metrics).await {
                crate::logs::error(
                    "outbox",
                    crate::campos! {
                        "msg": "Fallo al publicar eventos outbox",
                        "err": error.mensaje,
                    },
                );
            }
        }
    })
}

/// Worker de recordatorios cada 60 segundos.
pub fn iniciar_worker(agenda: Arc<dyn AgendaPort>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        ticker.tick().await; // setInterval: el primer ciclo es al minuto
        loop {
            ticker.tick().await;
            let request_id = format!(
                "scheduling-worker-{}",
                chrono::Utc::now().timestamp_millis()
            );
            let resultado = ejecutar_con_contexto(
                contexto(request_id, "scheduling-worker"),
                agenda.procesar_recordatorios(),
            )
            .await;
            if let Err(error) = resultado {
                crate::logs::error(
                    "scheduling",
                    crate::campos! {
                        "msg": "Fallo al procesar recordatorios",
                        "err": error.mensaje,
                    },
                );
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduling::{
        CrearBloqueo, CrearCita, CrearDisponibilidad, CrearRecurso, VerificarDisponibilidad,
    };
    use async_trait::async_trait;

    struct AgendaNula;

    #[async_trait]
    impl AgendaPort for AgendaNula {
        async fn crear_recurso(&self, _: &str, _: &CrearRecurso) -> Result<Value, ApiError> {
            Ok(Value::Null)
        }
        async fn listar_recursos(&self, _: &str) -> Result<Vec<Value>, ApiError> {
            Ok(vec![])
        }
        async fn crear_disponibilidad(
            &self,
            _: &str,
            _: &CrearDisponibilidad,
        ) -> Result<Value, ApiError> {
            Ok(Value::Null)
        }
        async fn listar_disponibilidad(&self, _: &str, _: &str) -> Result<Vec<Value>, ApiError> {
            Ok(vec![])
        }
        async fn crear_bloqueo(
            &self,
            _: &str,
            _: Option<&str>,
            _: &CrearBloqueo,
        ) -> Result<Value, ApiError> {
            Ok(Value::Null)
        }
        async fn crear_cita(&self, _: &str, _: &CrearCita) -> Result<Value, ApiError> {
            Ok(Value::Null)
        }
        async fn listar_citas(
            &self,
            _: &str,
            _: Option<&str>,
            _: Option<&str>,
        ) -> Result<Vec<Value>, ApiError> {
            Ok(vec![])
        }
        async fn cancelar_cita(
            &self,
            _: &str,
            _: &str,
            _: Option<&str>,
        ) -> Result<Value, ApiError> {
            Ok(Value::Null)
        }
        async fn listar_recordatorios(&self, _: &str) -> Result<Vec<Value>, ApiError> {
            Ok(vec![])
        }
        async fn crear_desde_reserva(&self, _: &OrdenReserva) -> Result<Value, ApiError> {
            Ok(Value::Null)
        }
        async fn procesar_recordatorios(&self) -> Result<usize, ApiError> {
            Ok(0)
        }
        async fn verificar_disponibilidad(
            &self,
            _: &VerificarDisponibilidad,
        ) -> Result<(), ApiError> {
            Ok(())
        }
    }

    #[test]
    fn debe_construir_el_contexto_del_consumidor() {
        let ctx = contexto("scheduling-event-1".to_string(), "scheduling-consumer");
        assert_eq!(ctx.servicio, "scheduling-consumer");
        assert_eq!(ctx.rol.as_deref(), Some("super_admin"));
        assert_eq!(ctx.usuario_id.as_deref(), Some(USUARIO_SERVICIO));
    }
}
