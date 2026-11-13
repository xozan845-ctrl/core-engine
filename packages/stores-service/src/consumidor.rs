//! Consumidor de `stores.stock` (`stock.updated`, RN-02).
//!
//! Sincroniza el stock visible de las ofertas de la empresa y deja la oferta
//! `activa`/`agotada`. La idempotencia es por `event_id` en
//! `stores.eventos_procesados` (doc 5.2).

use crate::db::Db;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::ofertas::Ofertas;
use futures_util::future::BoxFuture;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct StockConsumer {
    db: Arc<Db>,
    ofertas: Arc<Ofertas>,
}

impl StockConsumer {
    pub fn nuevo(db: Arc<Db>, ofertas: Arc<Ofertas>) -> Self {
        Self { db, ofertas }
    }

    pub async fn manejar(&self, evento: Value) -> Result<(), String> {
        if evento.get("tipo").and_then(Value::as_str) != Some("stock.updated") {
            return Ok(());
        }
        let event_id = evento
            .get("event_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "evento sin event_id".to_string())?;
        let contexto = ContextoCorrelacion {
            request_id: format!("stores-event-{event_id}"),
            servicio: "stores-consumer".to_string(),
            usuario_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
            rol: Some("super_admin".to_string()),
            tenant_id: None,
        };
        let consumer = self;
        ejecutar_con_contexto(contexto, async move {
            if consumer
                .ya_procesado(&event_id)
                .await
                .map_err(|e| e.to_string())?
            {
                return Ok(());
            }
            let items = evento
                .get("data")
                .and_then(|data| data.get("items"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for item in items {
                let empresa_id = item
                    .get("empresa_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let sku = item.get("sku").and_then(Value::as_str).unwrap_or_default();
                let stock = item
                    .get("stock_restante")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                consumer
                    .ofertas
                    .sincronizar_stock_de_oferta(empresa_id, sku, stock)
                    .await
                    .map_err(|e| e.mensaje)?;
            }
            consumer
                .marcar_procesado(
                    &event_id,
                    evento.get("tipo").and_then(Value::as_str).unwrap_or(""),
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok(())
        })
        .await
    }

    async fn ya_procesado(&self, event_id: &str) -> Result<bool, sqlx::Error> {
        let mut tx = self.db.transaccion().await?;
        let fila = sqlx::query("SELECT 1 FROM stores.eventos_procesados WHERE event_id = $1::uuid")
            .bind(event_id)
            .fetch_optional(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(fila.is_some())
    }

    async fn marcar_procesado(&self, event_id: &str, tipo: &str) -> Result<(), sqlx::Error> {
        let mut tx = self.db.transaccion().await?;
        sqlx::query(
            "INSERT INTO stores.eventos_procesados (event_id, tipo, procesado_en)
             VALUES ($1::uuid, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(tipo)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

/// Handler compatible con el cliente RabbitMQ.
pub fn handler_consumidor(consumidor: Arc<StockConsumer>) -> crate::rabbit::Handler {
    Arc::new(
        move |evento: Value| -> BoxFuture<'static, Result<(), String>> {
            let consumidor = Arc::clone(&consumidor);
            Box::pin(async move { consumidor.manejar(evento).await })
        },
    )
}

/// Evento mínimo usado por las pruebas y el log de arranque.
pub fn cola() -> crate::rabbit::ColaConfig {
    crate::rabbit::ColaConfig {
        nombre: "stores.stock".to_string(),
        routing_keys: vec!["stock.updated".to_string()],
    }
}

/// Payload de ejemplo para documentar el contrato del evento.
pub fn evento_ejemplo(event_id: &str) -> Value {
    json!({
        "event_id": event_id,
        "tipo": "stock.updated",
        "data": { "items": [] },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_declarar_la_cola_y_el_routing_del_contrato() {
        let cola = cola();
        assert_eq!(cola.nombre, "stores.stock");
        assert_eq!(cola.routing_keys, vec!["stock.updated"]);
        let evento = evento_ejemplo("130c07c6-7e10-4f91-9c13-e17ca6be6eea");
        assert_eq!(evento["tipo"], "stock.updated");
    }
}
