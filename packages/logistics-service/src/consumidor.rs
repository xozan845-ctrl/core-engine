//! Consumidor de `payment.procesado` y `order.status.updated` (Tabla 22).
//!
//! - `payment.procesado` prepara la guía de despacho y emite `shipment.started`.
//! - `order.status.updated` notifica; si `entregada` emite `order.completado`
//!   (comisiones devengan) y si `devuelta` emite `devolucion.solicitada`.
//!
//! Idempotencia por `event_id` en `logistics.eventos_procesados`.

use crate::db::Db;
use crate::envios::{Envio, EnviosPort};
use crate::errors::ApiError;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::outbox::{Outbox, DEVOLUCION_SOLICITADA, ORDER_COMPLETADO, SHIPMENT_STARTED};
use crate::rabbit::Rabbit;
use serde_json::{json, Map, Value};
use std::sync::Arc;

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

pub struct LogisticsConsumer {
    db: Arc<Db>,
    envios: Arc<dyn EnviosPort>,
    outbox: Outbox,
}

impl LogisticsConsumer {
    pub fn nuevo(db: Arc<Db>, envios: Arc<dyn EnviosPort>, outbox: Outbox) -> Self {
        Self { db, envios, outbox }
    }

    pub async fn manejar(&self, evento: Value) -> Result<(), ApiError> {
        let tipo = evento.get("tipo").and_then(Value::as_str).unwrap_or("");
        if tipo != "payment.procesado" && tipo != "order.status.updated" {
            return Ok(());
        }
        let event_id = evento
            .get("event_id")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
            .ok_or_else(|| ApiError::dominio("EVENTO_INVALIDO", "El evento no tiene event_id."))?
            .to_string();

        if self.ya_procesado(&event_id).await? {
            return Ok(());
        }

        if tipo == "payment.procesado" {
            self.pago_procesado(&evento).await?;
        } else {
            self.estado_actualizado(&evento).await?;
        }
        self.marcar_procesado(&event_id, tipo).await
    }

    async fn pago_procesado(&self, evento: &Value) -> Result<(), ApiError> {
        let data = evento.get("data").cloned().unwrap_or(Value::Null);
        let order_id = data
            .get("order_id")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
            .ok_or_else(|| ApiError::dominio("EVENTO_INVALIDO", "El evento no tiene order_id."))?
            .to_string();

        let envios = self.envios.preparar_envios_de_orden(&order_id).await?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::envios::error_db)?;
        for envio in &envios {
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    SHIPMENT_STARTED,
                    payload_shipment(&order_id, envio),
                )
                .await?;
        }
        tx.commit().await.map_err(crate::envios::error_db)?;
        self.notificar(
            "email",
            &order_id,
            "La bodega alista tu pedido",
            json!({"comprador": true, "vendedor": true}),
        );
        Ok(())
    }

    async fn estado_actualizado(&self, evento: &Value) -> Result<(), ApiError> {
        let data = evento.get("data").cloned().unwrap_or(Value::Null);
        let order_id = data
            .get("order_id")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
            .ok_or_else(|| ApiError::dominio("EVENTO_INVALIDO", "El evento no tiene order_id."))?
            .to_string();
        let estado = data
            .get("estado")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let motivo = data
            .get("motivo")
            .and_then(Value::as_str)
            .map(str::to_string);

        self.notificar(
            "email",
            &order_id,
            &format!("Tu pedido paso a estado \"{estado}\""),
            json!({"comprador": true, "vendedor": true}),
        );

        if estado == "entregada" {
            let mut tx = self
                .db
                .transaccion()
                .await
                .map_err(crate::envios::error_db)?;
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    ORDER_COMPLETADO,
                    json!({
                        "order_id": order_id,
                        "entregado_en": crate::metrics::ahora_iso(),
                    }),
                )
                .await?;
            tx.commit().await.map_err(crate::envios::error_db)?;
            crate::logs::info(
                "envios",
                crate::campos! {
                    "msg": "Orden completada: comision listo para devengar",
                    "order_id": order_id,
                },
            );
        } else if estado == "devuelta" {
            let items = match self.envios.consultar_orden(&order_id).await? {
                Some(orden) => orden
                    .items
                    .iter()
                    .map(|item| {
                        json!({
                            "empresa_id": item.empresa_id,
                            "sku": item.sku,
                            "cantidad": item.cantidad,
                        })
                    })
                    .collect::<Vec<Value>>(),
                None => Vec::new(),
            };
            let mut tx = self
                .db
                .transaccion()
                .await
                .map_err(crate::envios::error_db)?;
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    DEVOLUCION_SOLICITADA,
                    json!({
                        "order_id": order_id,
                        "motivo": motivo.unwrap_or_else(|| "devolucion aprobada".to_string()),
                        "items": items,
                    }),
                )
                .await?;
            tx.commit().await.map_err(crate::envios::error_db)?;
            crate::logs::info(
                "envios",
                crate::campos! {
                    "msg": "Devolucion solicitada: stock y comision se revertiran",
                    "order_id": order_id,
                },
            );
        }
        Ok(())
    }

    /// RF-07: notificaciones por correo/push (simuladas con logs estructurados).
    fn notificar(&self, canal: &str, order_id: &str, mensaje: &str, destinatarios: Value) {
        crate::logs::info(
            "envios",
            crate::campos! {
                "msg": "notificacion_enviada",
                "canal": canal,
                "order_id": order_id,
                "texto": mensaje,
                "destinatarios": destinatarios,
            },
        );
    }

    async fn ya_procesado(&self, event_id: &str) -> Result<bool, ApiError> {
        let id = crate::envios::parsear_uuid(event_id)?;
        let fila = sqlx::query("SELECT 1 FROM logistics.eventos_procesados WHERE event_id = $1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await
            .map_err(crate::envios::error_db)?;
        Ok(fila.is_some())
    }

    async fn marcar_procesado(&self, event_id: &str, tipo: &str) -> Result<(), ApiError> {
        let id = crate::envios::parsear_uuid(event_id)?;
        sqlx::query(
            "INSERT INTO logistics.eventos_procesados (event_id, tipo, procesado_en)
             VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(id)
        .bind(tipo)
        .execute(self.db.pool())
        .await
        .map_err(crate::envios::error_db)?;
        Ok(())
    }
}

/// Payload de `shipment.started` con el mismo shape y orden que el TypeScript.
fn payload_shipment(order_id: &str, envio: &Envio) -> Value {
    let mut data = Map::new();
    data.insert("order_id".into(), json!(order_id));
    data.insert("empresa_id".into(), json!(envio.empresa_id));
    if let Some(parte_id) = envio.parte_id.as_deref() {
        data.insert("parte_id".into(), json!(parte_id));
    }
    data.insert("guia".into(), json!(envio.guia));
    data.insert("estado".into(), json!("en_preparacion"));
    data.insert("proveedor_tipo".into(), json!(envio.proveedor_tipo));
    data.insert(
        "proveedor_tenant_id".into(),
        json!(envio.proveedor_tenant_id),
    );
    Value::Object(data)
}

/// Handler para `Rabbit::consumir`: envuelve el mensaje en el contexto del
/// servicio (igual que `ejecutarConContexto`).
pub fn handler_consumidor(consumidor: Arc<LogisticsConsumer>) -> crate::rabbit::Handler {
    Arc::new(move |evento| {
        let consumidor = Arc::clone(&consumidor);
        Box::pin(async move {
            let request_id = evento
                .get("event_id")
                .and_then(Value::as_str)
                .map(|id| format!("logistics-event-{id}"))
                .unwrap_or_else(|| "logistics-event-desconocido".to_string());
            ejecutar_con_contexto(contexto(request_id, "logistics-consumer"), async move {
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
    metrics: Arc<crate::metrics::Metrics>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(1));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn envio() -> Envio {
        Envio {
            id: "e-1".into(),
            guia: "BGH-ABC-123456".into(),
            order_id: "o-1".into(),
            monto_cents: 150_000,
            estado: "en_preparacion".into(),
            creado_en: "2026-09-01T00:00:00.000Z".into(),
            empresa_id: "empresa-1".into(),
            proveedor_tipo: Some("empresa".into()),
            proveedor_tenant_id: Some("empresa-1".into()),
            parte_id: None,
        }
    }

    #[test]
    fn debe_construir_el_payload_de_shipment_started() {
        let payload = payload_shipment("o-1", &envio());
        assert_eq!(payload["order_id"], "o-1");
        assert_eq!(payload["empresa_id"], "empresa-1");
        assert_eq!(payload["guia"], "BGH-ABC-123456");
        assert_eq!(payload["estado"], "en_preparacion");
        assert_eq!(payload["proveedor_tipo"], "empresa");
        assert!(payload.get("parte_id").is_none());
    }

    #[test]
    fn debe_incluir_parte_id_solo_cuando_existe() {
        let mut con_parte = envio();
        con_parte.parte_id = Some("parte-1".into());
        let payload = payload_shipment("o-1", &con_parte);
        assert_eq!(payload["parte_id"], "parte-1");
    }

    #[test]
    fn debe_construir_el_contexto_del_consumidor() {
        let ctx = contexto("logistics-event-1".to_string(), "logistics-consumer");
        assert_eq!(ctx.servicio, "logistics-consumer");
        assert_eq!(ctx.rol.as_deref(), Some("super_admin"));
        assert_eq!(ctx.usuario_id.as_deref(), Some(USUARIO_SERVICIO));
    }
}
