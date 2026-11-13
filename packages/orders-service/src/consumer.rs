//! Consumidor de resultados de stock (Tabla 22):
//! - `stock.reservado` → la orden pasa a `pagada` y emite `payment.procesado`
//! - `stock.fallido` → la orden se cancela (RN-03)
//!   Idempotencia por `event_id` en `orders.eventos_procesados`.

use crate::checkout::{Checkout, PAYMENT_PROCESADO};
use crate::db::Db;
use crate::event_store::parsear_uuid;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;

pub const STOCK_RESERVADO: &str = "stock.reservado";
pub const STOCK_FALLIDO: &str = "stock.fallido";

pub struct PedidosConsumer {
    db: Arc<Db>,
    checkout: Arc<Checkout>,
}

impl PedidosConsumer {
    pub fn nuevo(db: Arc<Db>, checkout: Arc<Checkout>) -> Self {
        Self { db, checkout }
    }

    pub async fn manejar(&self, evento: Value) -> Result<(), String> {
        let tipo = evento
            .get("tipo")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let event_id = evento
            .get("event_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if !matches!(tipo.as_str(), STOCK_RESERVADO | STOCK_FALLIDO) {
            return Ok(());
        }
        let contexto = ContextoCorrelacion {
            request_id: format!("orders-event-{event_id}"),
            servicio: "orders-consumer".to_string(),
            usuario_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
            rol: Some("super_admin".to_string()),
            tenant_id: None,
        };
        ejecutar_con_contexto(contexto, async {
            match tipo.as_str() {
                STOCK_RESERVADO => self.stock_reservado(&evento).await,
                STOCK_FALLIDO => self.stock_fallido(&evento).await,
                _ => Ok(()),
            }
        })
        .await
    }

    async fn stock_reservado(&self, evento: &Value) -> Result<(), String> {
        if self.ya_procesado(evento).await? {
            return Ok(());
        }
        let order_id = order_id_de(evento)?;
        let actual = sqlx::query(
            "SELECT estado, total_cents, metodo_pago FROM orders.orden_vista WHERE id = $1",
        )
        .bind(parsear_uuid(&order_id).map_err(|e| e.mensaje)?)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|e| e.to_string())?;
        if let Some(actual) = actual {
            let estado: String = actual.try_get("estado").map_err(|e| e.to_string())?;
            if estado == "creada" {
                let total = i64::from(
                    actual
                        .try_get::<i32, _>("total_cents")
                        .map_err(|e| e.to_string())?,
                );
                let metodo: String = actual.try_get("metodo_pago").map_err(|e| e.to_string())?;
                let es_credito = metodo == "credito";
                let pago = serde_json::json!({
                    "order_id": order_id,
                    "monto_cents": total,
                    "estado": "procesado",
                    "metodo": if es_credito { "credito" } else { "simulado" },
                });
                self.checkout
                    .transicionar(
                        &order_id,
                        "pagada",
                        Some(if es_credito {
                            "pago_credito_aprobado"
                        } else {
                            "pago_simulado_aprobado"
                        }),
                        Some("inventario"),
                        Some((PAYMENT_PROCESADO, pago)),
                    )
                    .await
                    .map_err(|e| e.mensaje)?;
            }
        }
        self.marcar_procesado(evento).await
    }

    async fn stock_fallido(&self, evento: &Value) -> Result<(), String> {
        if self.ya_procesado(evento).await? {
            return Ok(());
        }
        let order_id = order_id_de(evento)?;
        let actual = sqlx::query("SELECT estado FROM orders.orden_vista WHERE id = $1")
            .bind(parsear_uuid(&order_id).map_err(|e| e.mensaje)?)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| e.to_string())?;
        if let Some(actual) = actual {
            let estado: String = actual.try_get("estado").map_err(|e| e.to_string())?;
            if estado == "creada" {
                let skus = evento
                    .get("data")
                    .and_then(|data| data.get("items"))
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.get("sku").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                self.checkout
                    .transicionar(
                        &order_id,
                        "cancelada",
                        Some(&format!("stock_insuficiente: {skus}")),
                        Some("inventario"),
                        None,
                    )
                    .await
                    .map_err(|e| e.mensaje)?;
            }
        }
        self.marcar_procesado(evento).await
    }

    async fn ya_procesado(&self, evento: &Value) -> Result<bool, String> {
        let event_id = event_id_de(evento)?;
        let fila = sqlx::query("SELECT 1 FROM orders.eventos_procesados WHERE event_id = $1")
            .bind(parsear_uuid(&event_id).map_err(|e| e.mensaje)?)
            .fetch_optional(self.db.pool())
            .await
            .map_err(|e| e.to_string())?;
        Ok(fila.is_some())
    }

    async fn marcar_procesado(&self, evento: &Value) -> Result<(), String> {
        let event_id = event_id_de(evento)?;
        let tipo = evento
            .get("tipo")
            .and_then(Value::as_str)
            .unwrap_or_default();
        sqlx::query(
            "INSERT INTO orders.eventos_procesados (event_id, tipo, procesado_en)
             VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(parsear_uuid(&event_id).map_err(|e| e.mensaje)?)
        .bind(tipo)
        .execute(self.db.pool())
        .await
        .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn event_id_de(evento: &Value) -> Result<String, String> {
    evento
        .get("event_id")
        .and_then(Value::as_str)
        .filter(|valor| !valor.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| "event_id requerido".to_string())
}

fn order_id_de(evento: &Value) -> Result<String, String> {
    evento
        .get("data")
        .and_then(|data| data.get("order_id"))
        .and_then(Value::as_str)
        .filter(|valor| !valor.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| "order_id requerido".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_extraer_los_identificadores_del_evento() {
        let evento = serde_json::json!({
            "event_id": "123e4567-e89b-12d3-a456-426614174000",
            "data": {"order_id": "123e4567-e89b-12d3-a456-426614174001"}
        });
        assert_eq!(
            order_id_de(&evento).expect("order"),
            "123e4567-e89b-12d3-a456-426614174001"
        );
        assert_eq!(
            event_id_de(&evento).expect("event"),
            "123e4567-e89b-12d3-a456-426614174000"
        );
    }

    #[test]
    fn debe_rechazar_un_evento_sin_order_id() {
        assert!(order_id_de(&serde_json::json!({"data": {}})).is_err());
    }
}
