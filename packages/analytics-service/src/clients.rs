//! Cliente interno de órdenes (fuente de hechos).

use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ItemOrden {
    pub oferta_id: String,
    #[serde(default)]
    pub vendedor_id: Option<String>,
    #[serde(default)]
    pub empresa_id: Option<String>,
    #[serde(default)]
    pub negocio_id: Option<String>,
    #[serde(default)]
    pub tienda_id: Option<String>,
    #[serde(default)]
    pub participacion_porcentaje: Option<i64>,
    pub precio_unitario_cents: i64,
    pub cantidad: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OrdenInterna {
    #[serde(default)]
    pub comprador_id: Option<String>,
    #[serde(default)]
    pub creado_en: Option<String>,
    #[serde(default)]
    pub items: Vec<ItemOrden>,
}

#[async_trait]
pub trait Ordenes: Send + Sync {
    async fn obtener(&self, order_id: &str) -> Option<OrdenInterna>;
}

#[derive(Clone)]
pub struct OrdenesHttp {
    url: String,
    clave: String,
    timeout: Duration,
    cliente: reqwest::Client,
}

impl OrdenesHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
            cliente: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Ordenes for OrdenesHttp {
    async fn obtener(&self, order_id: &str) -> Option<OrdenInterna> {
        let respuesta = self
            .cliente
            .get(format!("{}/internal/orders/{order_id}", self.url))
            .header("x-internal-key", &self.clave)
            .timeout(self.timeout)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        respuesta.json::<OrdenInterna>().await.ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_deserializar_una_orden_con_lineas() {
        let orden: OrdenInterna = serde_json::from_value(serde_json::json!({
            "comprador_id": "33333333-3333-3333-3333-333333333333",
            "creado_en": "2026-11-01T10:00:00Z",
            "items": [{
                "oferta_id": "44444444-4444-4444-4444-444444444444",
                "empresa_id": "11111111-1111-1111-1111-111111111111",
                "negocio_id": "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
                "tienda_id": "55555555-5555-5555-5555-555555555555",
                "vendedor_id": "22222222-2222-2222-2222-222222222222",
                "precio_unitario_cents": 1000,
                "cantidad": 2,
                "participacion_porcentaje": 60
            }]
        }))
        .expect("orden");
        assert_eq!(orden.items.len(), 1);
        assert_eq!(orden.items[0].cantidad, 2);
        assert_eq!(
            orden.comprador_id.as_deref(),
            Some("33333333-3333-3333-3333-333333333333")
        );
    }
}
