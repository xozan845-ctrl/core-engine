//! Cliente interno de `orders-service` (REST, `x-internal-key` y timeout).
//!
//! Réplica de `consultarOrden` y `avanzarEstadoOrden` del
//! `EnviosService` TypeScript.

use crate::errors::ApiError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OrdenItemExterna {
    #[serde(default)]
    pub empresa_id: String,
    pub sku: String,
    pub cantidad: i64,
    pub producto_nombre: String,
    #[serde(default)]
    pub precio_unitario_cents: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OrdenExterna {
    pub id: String,
    pub comprador_id: String,
    #[serde(default)]
    pub parte_id: Option<String>,
    #[serde(default)]
    pub items: Vec<OrdenItemExterna>,
    pub total: String,
    pub total_cents: i64,
    pub estado: String,
}

#[derive(Debug, Deserialize)]
struct ErrorOrden {
    codigo: Option<String>,
    mensaje: Option<String>,
}

/// Operaciones internas de pedidos usadas por logística.
#[async_trait]
pub trait Orders: Send + Sync {
    /// `Ok(None)` cuando la orden no existe o orders no está disponible
    /// (mismo comportamiento que `consultarOrden`).
    async fn consultar(&self, order_id: &str) -> Result<Option<OrdenExterna>, ()>;

    /// Transición de estado; propaga el código y mensaje de orders.
    async fn avanzar(
        &self,
        order_id: &str,
        estado: &str,
        motivo: Option<&str>,
        actor: Option<&str>,
    ) -> Result<OrdenExterna, ApiError>;
}

pub struct OrdersHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl OrdersHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    fn cliente(&self) -> Result<reqwest::Client, ()> {
        reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| ())
    }
}

#[async_trait]
impl Orders for OrdersHttp {
    async fn consultar(&self, order_id: &str) -> Result<Option<OrdenExterna>, ()> {
        let cliente = self.cliente()?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/orders/{}",
                self.url,
                urlencoding::encode(order_id)
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .map_err(|_| ())?;
        if !respuesta.status().is_success() {
            return Ok(None);
        }
        respuesta
            .json::<OrdenExterna>()
            .await
            .map(Some)
            .map_err(|_| ())
    }

    async fn avanzar(
        &self,
        order_id: &str,
        estado: &str,
        motivo: Option<&str>,
        actor: Option<&str>,
    ) -> Result<OrdenExterna, ApiError> {
        let mut query = format!("estado={}", urlencoding::encode(estado));
        if let Some(motivo) = motivo {
            query.push_str(&format!("&motivo={}", urlencoding::encode(motivo)));
        }
        if let Some(actor) = actor {
            query.push_str(&format!("&actor={}", urlencoding::encode(actor)));
        }
        let url = format!(
            "{}/internal/orders/{}/transicion?{query}",
            self.url,
            urlencoding::encode(order_id)
        );

        let cliente = self
            .cliente()
            .map_err(|_| ApiError::servicio_no_disponible("orders-service"))?;
        let respuesta = cliente
            .post(url)
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("orders-service"))?;

        if !respuesta.status().is_success() {
            let cuerpo = respuesta.json::<ErrorOrden>().await.unwrap_or(ErrorOrden {
                codigo: None,
                mensaje: None,
            });
            return Err(ApiError::dominio(
                cuerpo
                    .codigo
                    .unwrap_or_else(|| "TRANSICION_FALLIDA".to_string()),
                cuerpo
                    .mensaje
                    .unwrap_or_else(|| "La transicion de estado fue rechazada.".to_string()),
            ));
        }

        respuesta.json::<OrdenExterna>().await.map_err(|_| {
            ApiError::dominio(
                "TRANSICION_FALLIDA",
                "La transicion de estado fue rechazada.",
            )
        })
    }
}

/// Implementación para tests: orders no disponible.
pub struct OrdersNoDisponible;

#[async_trait]
impl Orders for OrdersNoDisponible {
    async fn consultar(&self, _order_id: &str) -> Result<Option<OrdenExterna>, ()> {
        Ok(None)
    }

    async fn avanzar(
        &self,
        _order_id: &str,
        _estado: &str,
        _motivo: Option<&str>,
        _actor: Option<&str>,
    ) -> Result<OrdenExterna, ApiError> {
        Err(ApiError::servicio_no_disponible("orders-service"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn debe_degradar_consultar_y_avanzar_sin_orders() {
        let cliente = OrdersNoDisponible;
        assert_eq!(cliente.consultar("o-1").await, Ok(None));
        let error = cliente
            .avanzar("o-1", "enviada", None, None)
            .await
            .expect_err("503");
        assert_eq!(error.codigo, "SERVICIO_NO_DISPONIBLE");
        assert_eq!(error.status, axum::http::StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn debe_deserializar_los_items_internos_de_la_orden() {
        let orden: OrdenExterna = serde_json::from_value(serde_json::json!({
            "id": "o-1",
            "comprador_id": "c-1",
            "parte_id": null,
            "items": [{
                "empresa_id": "e-1",
                "sku": "SKU-1",
                "cantidad": 2,
                "producto_nombre": "Cafe",
                "precio_unitario_cents": 1500
            }],
            "total": "30.00",
            "total_cents": 3000,
            "estado": "pagada"
        }))
        .expect("orden");
        assert_eq!(orden.items[0].precio_unitario_cents, 1500);
        assert_eq!(orden.parte_id, None);
    }
}
