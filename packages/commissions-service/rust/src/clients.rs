//! Clientes internos (servicio→servicio) con timeout y fallo controlado.

use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ItemOrden {
    pub oferta_id: String,
    pub vendedor_id: Option<String>,
    pub empresa_id: Option<String>,
    pub tienda_id: Option<String>,
    pub participacion_porcentaje: Option<i64>,
    pub precio_unitario_cents: i64,
    pub cantidad: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct OrdenInterna {
    #[serde(default)]
    pub total_cents: i64,
    #[serde(default)]
    pub items: Vec<ItemOrden>,
}

/// Consulta de la orden completada (orders, superficie interna).
#[async_trait]
pub trait Ordenes: Send + Sync {
    /// `None` si la orden no está disponible, el servicio falla o el cuerpo no
    /// es válido; el consumidor lo traduce a error para reintento.
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InventarioResumen {
    pub productos: i64,
    pub stock_total: i64,
    pub agotados: i64,
}

/// KPIs externos de catálogo e identidad; un fallo degrada a cero.
#[async_trait]
pub trait Kpis: Send + Sync {
    async fn inventario(&self) -> InventarioResumen;
    async fn vendedores(&self) -> i64;
}

#[derive(Deserialize)]
struct RespuestaInventario {
    #[serde(default)]
    productos: i64,
    #[serde(default)]
    stock_total: i64,
    #[serde(default)]
    agotados: i64,
}

#[derive(Deserialize)]
struct RespuestaVendedores {
    #[serde(default)]
    vendedores: i64,
}

#[derive(Clone)]
pub struct KpisHttp {
    catalogo_url: String,
    identidad_url: String,
    clave: String,
    timeout: Duration,
    cliente: reqwest::Client,
}

impl KpisHttp {
    pub fn nueva(
        catalogo_url: String,
        identidad_url: String,
        clave: Option<String>,
        timeout_ms: u64,
    ) -> Self {
        Self {
            catalogo_url,
            identidad_url,
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
            cliente: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Kpis for KpisHttp {
    async fn inventario(&self) -> InventarioResumen {
        let respuesta = match self
            .cliente
            .get(format!("{}/internal/inventario/resumen", self.catalogo_url))
            .header("x-internal-key", &self.clave)
            .timeout(self.timeout)
            .send()
            .await
        {
            Ok(respuesta) if respuesta.status().is_success() => respuesta,
            _ => return InventarioResumen::default(),
        };
        respuesta
            .json::<RespuestaInventario>()
            .await
            .map(|cuerpo| InventarioResumen {
                productos: cuerpo.productos,
                stock_total: cuerpo.stock_total,
                agotados: cuerpo.agotados,
            })
            .unwrap_or_default()
    }

    async fn vendedores(&self) -> i64 {
        let respuesta = match self
            .cliente
            .get(format!("{}/internal/contar-vendedores", self.identidad_url))
            .header("x-internal-key", &self.clave)
            .timeout(self.timeout)
            .send()
            .await
        {
            Ok(respuesta) if respuesta.status().is_success() => respuesta,
            _ => return 0,
        };
        respuesta
            .json::<RespuestaVendedores>()
            .await
            .map(|cuerpo| cuerpo.vendedores)
            .unwrap_or(0)
    }
}

/// Implementación para tests: catálogo e identidad no disponibles.
pub struct KpisNoDisponible;

#[async_trait]
impl Kpis for KpisNoDisponible {
    async fn inventario(&self) -> InventarioResumen {
        InventarioResumen::default()
    }

    async fn vendedores(&self) -> i64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_deserializar_una_orden_con_items_completos() {
        let orden: OrdenInterna = serde_json::from_value(serde_json::json!({
            "total_cents": 1000,
            "items": [{
                "oferta_id": "of-1",
                "vendedor_id": "ven-1",
                "empresa_id": "emp-1",
                "tienda_id": "tie-1",
                "participacion_porcentaje": 70,
                "precio_unitario_cents": 500,
                "cantidad": 2
            }]
        }))
        .expect("orden");
        assert_eq!(orden.items.len(), 1);
        assert_eq!(orden.items[0].participacion_porcentaje, Some(70));
    }

    #[test]
    fn debe_tolerar_una_orden_sin_items() {
        let orden: OrdenInterna =
            serde_json::from_value(serde_json::json!({ "total_cents": 0 })).expect("orden");
        assert!(orden.items.is_empty());
    }

    #[test]
    fn debe_degradar_los_kpis_a_cero_sin_servicios() {
        let kpis = KpisNoDisponible;
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        runtime.block_on(async {
            assert_eq!(kpis.vendedores().await, 0);
            assert_eq!(kpis.inventario().await, InventarioResumen::default());
        });
    }
}
