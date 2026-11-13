//! Consumidor multi-cola de inteligencia de mercado (3 topics del bus).

use crate::agregador::AgregadorPort;
use crate::calidad::CalidadDatos;
use crate::db::Db;
use crate::errors::ApiError;
use crate::inteligencia::{error_db, parsear_uuid, VentaGeolocalizada};
use crate::logs;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::rabbit::Rabbit;
use serde_json::Value;
use std::sync::Arc;

pub const COLA_GEO: &str = "intelligence.venta_geo";
pub const COLA_ORDERS: &str = "intelligence.order_completado";
pub const COLA_STOCK: &str = "intelligence.stock_updated";

const USUARIO_SERVICIO: &str = "00000000-0000-0000-0000-000000000000";

fn contexto(request_id: String) -> ContextoCorrelacion {
    ContextoCorrelacion {
        request_id,
        servicio: "intelligence-consumer".to_string(),
        usuario_id: Some(USUARIO_SERVICIO.to_string()),
        rol: Some("super_admin".to_string()),
        tenant_id: None,
    }
}

pub struct InteligenciaConsumer {
    db: Arc<Db>,
    agregador: Arc<dyn AgregadorPort>,
    calidad: Arc<CalidadDatos>,
}

impl InteligenciaConsumer {
    pub fn nuevo(
        db: Arc<Db>,
        agregador: Arc<dyn AgregadorPort>,
        calidad: Arc<CalidadDatos>,
    ) -> Self {
        Self {
            db,
            agregador,
            calidad,
        }
    }

    pub async fn manejar_venta(&self, evento: Value) -> Result<(), ApiError> {
        if evento.get("tipo").and_then(Value::as_str) != Some("venta.geolocalizada") {
            return Ok(());
        }
        let event_id = event_id(&evento)?;
        if self.ya_procesado(&event_id).await? {
            return Ok(());
        }

        let data = evento.get("data").cloned().unwrap_or(Value::Null);
        let calidad = self.calidad.validar(&data);
        if !calidad.is_valid {
            logs::error(
                "inteligencia.consumer",
                crate::campos! {
                    "msg": "Evento venta.geolocalizada rechazado por Data Contract",
                    "event_id": event_id,
                    "errors": calidad.errors,
                },
            );
            self.calidad
                .registrar_invalido(
                    Some(&event_id),
                    "venta.geolocalizada",
                    &calidad.errors,
                    &data,
                )
                .await;
            self.marcar_procesado(&event_id, "venta.geolocalizada")
                .await?;
            return Ok(());
        }

        let venta: VentaGeolocalizada = serde_json::from_value(data).map_err(|error| {
            ApiError::dominio("EVENTO_INVALIDO", format!("Evento invalido: {error}"))
        })?;
        self.agregador
            .procesar_venta_geolocalizada(&venta, Some(&event_id))
            .await?;
        self.marcar_procesado(&event_id, "venta.geolocalizada")
            .await?;
        logs::info(
            "inteligencia.consumer",
            crate::campos! {
                "msg": "venta.geolocalizada procesada",
                "event_id": event_id,
                "qualityTier": calidad.quality_tier,
            },
        );
        Ok(())
    }

    /// La cola `intelligence.order_completado` consume `comision.acreditada`
    /// porque es el evento que trae `vendedor_id` (igual que el TypeScript).
    pub async fn manejar_comision(&self, evento: Value) -> Result<(), ApiError> {
        if evento.get("tipo").and_then(Value::as_str) != Some("comision.acreditada") {
            return Ok(());
        }
        let event_id = event_id(&evento)?;
        if self.ya_procesado(&event_id).await? {
            return Ok(());
        }
        let data = evento.get("data").cloned().unwrap_or(Value::Null);
        let order_id = texto_requerido(&data, "order_id")?;
        let vendedor_id = texto_requerido(&data, "vendedor_id")?;
        self.agregador
            .procesar_orden_completada(&order_id, &vendedor_id)
            .await?;
        self.marcar_procesado(&event_id, "comision.acreditada")
            .await?;
        logs::info(
            "inteligencia.consumer",
            crate::campos! { "msg": "comision.acreditada procesada", "event_id": event_id },
        );
        Ok(())
    }

    pub async fn manejar_stock(&self, evento: Value) -> Result<(), ApiError> {
        if evento.get("tipo").and_then(Value::as_str) != Some("stock.updated") {
            return Ok(());
        }
        let event_id = event_id(&evento)?;
        if self.ya_procesado(&event_id).await? {
            return Ok(());
        }
        let data = evento.get("data").cloned().unwrap_or(Value::Null);
        let items = data
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for item in &items {
            let empresa_id = texto_requerido(item, "empresa_id")?;
            let sku = texto_requerido(item, "sku")?;
            let stock = item
                .get("stock_restante")
                .and_then(Value::as_i64)
                .ok_or_else(|| {
                    ApiError::dominio("EVENTO_INVALIDO", "El item no tiene stock_restante.")
                })?;
            self.agregador
                .procesar_stock_actualizado(&empresa_id, &sku, stock as i32, None)
                .await?;
        }
        self.marcar_procesado(&event_id, "stock.updated").await?;
        logs::info(
            "inteligencia.consumer",
            crate::campos! { "msg": "stock.updated procesado", "event_id": event_id },
        );
        Ok(())
    }

    async fn ya_procesado(&self, event_id: &str) -> Result<bool, ApiError> {
        let id = parsear_uuid(event_id)?;
        let fila = sqlx::query("SELECT 1 FROM intelligence.eventos_procesados WHERE event_id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
        if fila.is_some() {
            logs::warn(
                "inteligencia.consumer",
                crate::campos! { "msg": "Evento duplicado ignorado", "event_id": event_id },
            );
        }
        Ok(fila.is_some())
    }

    async fn marcar_procesado(&self, event_id: &str, tipo: &str) -> Result<(), ApiError> {
        let id = parsear_uuid(event_id)?;
        sqlx::query(
            "INSERT INTO intelligence.eventos_procesados (event_id, tipo, procesado_en)
             VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(id)
        .bind(tipo)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(())
    }
}

fn event_id(evento: &Value) -> Result<String, ApiError> {
    evento
        .get("event_id")
        .and_then(Value::as_str)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("EVENTO_INVALIDO", "El evento no tiene event_id."))
}

fn texto_requerido(valor: &Value, campo: &str) -> Result<String, ApiError> {
    valor
        .get(campo)
        .and_then(Value::as_str)
        .filter(|texto| !texto.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("EVENTO_INVALIDO", format!("El evento no tiene {campo}.")))
}

pub fn handler_venta(consumer: Arc<InteligenciaConsumer>) -> crate::rabbit::Handler {
    Arc::new(move |evento| {
        let consumer = Arc::clone(&consumer);
        Box::pin(async move {
            let request_id = request_id(&evento, "venta.geolocalizada");
            ejecutar_con_contexto(contexto(request_id), async move {
                consumer
                    .manejar_venta(evento)
                    .await
                    .map_err(|error| error.mensaje)
            })
            .await
        })
    })
}

pub fn handler_comision(consumer: Arc<InteligenciaConsumer>) -> crate::rabbit::Handler {
    Arc::new(move |evento| {
        let consumer = Arc::clone(&consumer);
        Box::pin(async move {
            let request_id = request_id(&evento, "comision.acreditada");
            ejecutar_con_contexto(contexto(request_id), async move {
                consumer
                    .manejar_comision(evento)
                    .await
                    .map_err(|error| error.mensaje)
            })
            .await
        })
    })
}

pub fn handler_stock(consumer: Arc<InteligenciaConsumer>) -> crate::rabbit::Handler {
    Arc::new(move |evento| {
        let consumer = Arc::clone(&consumer);
        Box::pin(async move {
            let request_id = request_id(&evento, "stock.updated");
            ejecutar_con_contexto(contexto(request_id), async move {
                consumer
                    .manejar_stock(evento)
                    .await
                    .map_err(|error| error.mensaje)
            })
            .await
        })
    })
}

fn request_id(evento: &Value, etiqueta: &str) -> String {
    evento
        .get("event_id")
        .and_then(Value::as_str)
        .map(|id| format!("intelligence-event-{id}"))
        .unwrap_or_else(|| format!("intelligence-event-{etiqueta}"))
}

/// Publica el Outbox cada segundo (el servicio no emite eventos, pero el
/// módulo compartido mantiene el poller activo como el TypeScript).
pub fn iniciar_outbox(
    outbox: crate::outbox::Outbox,
    db: Arc<Db>,
    rabbit: Arc<Rabbit>,
    metrics: Arc<crate::metrics::Metrics>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(1));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if let Err(error) = outbox.publicar_pendientes(&db, &rabbit, &metrics).await {
                logs::error(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_construir_el_contexto_del_consumidor() {
        let ctx = contexto("intelligence-event-1".to_string());
        assert_eq!(ctx.servicio, "intelligence-consumer");
        assert_eq!(ctx.rol.as_deref(), Some("super_admin"));
        assert_eq!(ctx.usuario_id.as_deref(), Some(USUARIO_SERVICIO));
    }

    #[test]
    fn debe_exigir_event_id_y_campos_de_texto() {
        let error = event_id(&serde_json::json!({})).expect_err("invalido");
        assert_eq!(error.codigo, "EVENTO_INVALIDO");
        let valor = serde_json::json!({"order_id": "o-1"});
        assert_eq!(texto_requerido(&valor, "order_id").expect("order"), "o-1");
        assert!(texto_requerido(&valor, "vendedor_id").is_err());
    }
}
