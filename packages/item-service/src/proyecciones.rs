//! Clientes internos para las proyecciones que ya no viven en `item.items`:
//! precio base en `pricing-service` y stock inicial en `inventory-service`.

use crate::errors::ApiError;
use serde_json::json;
use std::time::Duration;

#[derive(Clone)]
pub struct Proyecciones {
    pricing_url: String,
    inventory_url: String,
    clave: String,
    timeout: Duration,
    cliente: reqwest::Client,
}

impl Proyecciones {
    pub fn nueva(
        pricing_url: String,
        inventory_url: String,
        clave: String,
        timeout_ms: u64,
    ) -> Self {
        Self {
            pricing_url: pricing_url.trim_end_matches('/').to_string(),
            inventory_url: inventory_url.trim_end_matches('/').to_string(),
            clave,
            timeout: Duration::from_millis(timeout_ms.max(1)),
            cliente: reqwest::Client::new(),
        }
    }

    pub async fn fijar_precio(
        &self,
        empresa_id: &str,
        item_id: &str,
        precio_cents: i64,
    ) -> Result<(), ApiError> {
        let respuesta = self
            .cliente
            .put(format!(
                "{}/internal/items/{}/precio-base",
                self.pricing_url,
                urlencoding::encode(item_id)
            ))
            .header("x-internal-key", &self.clave)
            .timeout(self.timeout)
            .json(&json!({
                "empresa_id": empresa_id,
                "precio_cents": precio_cents,
            }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("pricing-service"))?;
        if respuesta.status().is_success() {
            Ok(())
        } else {
            Err(ApiError::servicio_no_disponible("pricing-service"))
        }
    }

    pub async fn stock_inicial(
        &self,
        empresa_id: &str,
        item_id: &str,
        sku: &str,
        cantidad: i64,
        negocio_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let respuesta = self
            .cliente
            .post(format!(
                "{}/internal/inventario/stock/inicial",
                self.inventory_url
            ))
            .header("x-internal-key", &self.clave)
            .timeout(self.timeout)
            .json(&json!({
                "empresa_id": empresa_id,
                "item_id": item_id,
                "sku": sku,
                "cantidad": cantidad,
                "negocio_id": negocio_id,
            }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("inventory-service"))?;
        if respuesta.status().is_success() {
            Ok(())
        } else {
            Err(ApiError::servicio_no_disponible("inventory-service"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::post, Router};

    async fn servidor() -> String {
        let app = Router::new()
            .route(
                "/internal/items/{id}/precio-base",
                axum::routing::put(|| async { axum::Json(json!({"ok": true})) }),
            )
            .route(
                "/internal/inventario/stock/inicial",
                post(|| async { axum::Json(json!({"ok": true})) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("servidor");
        });
        format!("http://{}", addr)
    }

    #[tokio::test]
    async fn debe_fijar_precio_y_stock_por_api_interna() {
        let url = servidor().await;
        let proyecciones = Proyecciones::nueva(url.clone(), url, "clave".into(), 1000);
        proyecciones
            .fijar_precio("empresa", "item", 1000)
            .await
            .expect("precio");
        proyecciones
            .stock_inicial("empresa", "item", "SKU", 5, None)
            .await
            .expect("stock");
    }

    #[tokio::test]
    async fn debe_degradar_si_los_servicios_no_responden() {
        let proyecciones = Proyecciones::nueva(
            "http://127.0.0.1:9".into(),
            "http://127.0.0.1:9".into(),
            String::new(),
            100,
        );
        let error = proyecciones
            .fijar_precio("empresa", "item", 1)
            .await
            .expect_err("precio");
        assert_eq!(error.codigo, "SERVICIO_NO_DISPONIBLE");
        let error = proyecciones
            .stock_inicial("empresa", "item", "SKU", 1, None)
            .await
            .expect_err("stock");
        assert_eq!(error.codigo, "SERVICIO_NO_DISPONIBLE");
    }
}
