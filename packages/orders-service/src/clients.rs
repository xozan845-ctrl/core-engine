//! Clientes internos (servicio→servicio): stores, catalog, identity, scheduling
//! y finance. Réplica de `@core/shared/clients/service-clients.ts`.
//!
//! Todas las llamadas usan `x-internal-key` y un timeout corto; el llamador
//! decide si un fallo se traduce a 503 (fail-closed) o a degradación.

use crate::config::Config;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct Clientes {
    http: reqwest::Client,
    stores_url: String,
    catalog_url: String,
    identity_url: String,
    scheduling_url: String,
    finance_url: String,
    clave: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Capacidad {
    pub empresa_id: String,
    pub capacidad_codigo: String,
    pub activo: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OfertaExterna {
    pub id: String,
    pub tienda_id: String,
    pub vendedor_id: String,
    pub empresa_id: String,
    #[serde(default)]
    pub participacion_porcentaje: Option<i64>,
    pub item_type: String,
    pub item_id: String,
    pub sku: String,
    pub producto_nombre: String,
    #[serde(default)]
    pub atributos: Option<Value>,
    pub margen: i64,
    pub precio_base: String,
    pub precio_venta: String,
    #[serde(default)]
    pub stock: Option<i64>,
    pub estado: String,
    #[serde(default)]
    pub moneda: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PrecioLinea {
    pub clave: String,
    pub origen: String,
    pub precio_unitario_cents: i64,
    #[serde(default)]
    pub precio_total_cents: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PreciosExternos {
    Ok(Vec<PrecioLinea>),
    NoEncontrado,
    Error,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Cotizacion {
    pub total_cents: i64,
    #[serde(default)]
    pub dias: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ItemStock {
    #[serde(default)]
    pub empresa_id: String,
    #[serde(default)]
    pub sku: String,
    #[serde(default)]
    pub stock: i64,
    #[serde(default)]
    pub track_serial: bool,
    #[serde(default)]
    pub seriales_disponibles: i64,
    #[serde(default)]
    pub estado: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreditoEstado {
    Ok,
    Rechazado,
    Error,
}

/// Puerto de integraciones internas. Permite dobles deterministas en pruebas
/// sin levantar stores/catalog/identity/scheduling/finance.
#[async_trait]
pub trait ClientesPort: Send + Sync {
    async fn capacidades(
        &self,
        empresas: &[String],
        capacidades: &[String],
    ) -> Option<Vec<Capacidad>>;
    async fn ofertas(&self, ids: &[String]) -> Vec<OfertaExterna>;
    async fn precios(
        &self,
        empresa_id: &str,
        lineas: Vec<Value>,
        lista_codigo: Option<&str>,
        segmento_codigo: Option<&str>,
    ) -> PreciosExternos;
    async fn cotizacion(
        &self,
        empresa_id: &str,
        item_id: &str,
        desde: &str,
        hasta: &str,
    ) -> Option<Cotizacion>;
    async fn disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
        inicia_en: &str,
        termina_en: &str,
    ) -> Option<bool>;
    async fn registrar_credito(
        &self,
        empresa_id: &str,
        parte_id: &str,
        order_id: &str,
        monto_cents: i64,
        concepto: &str,
        vendedor_id: Option<&str>,
    ) -> CreditoEstado;
    async fn anular_credito(&self, order_id: &str) -> bool;
    async fn items_lote(&self, skus: &[String]) -> Option<Vec<ItemStock>>;
}

impl Clientes {
    pub fn nueva(config: &Config) -> Arc<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_millis(config.internal_request_timeout_ms))
            .build()
            .expect("cliente HTTP interno");
        Arc::new(Self {
            http,
            stores_url: config.stores_service_url.clone(),
            catalog_url: config.catalog_service_url.clone(),
            identity_url: config.identity_service_url.clone(),
            scheduling_url: config.scheduling_service_url.clone(),
            finance_url: config.finance_service_url.clone(),
            clave: config.internal_api_key.clone().unwrap_or_default(),
        })
    }

    /// `None` = identity no disponible (fail-closed en el llamador).
    pub async fn capacidades(
        &self,
        empresas: &[String],
        capacidades: &[String],
    ) -> Option<Vec<Capacidad>> {
        if empresas.is_empty() || capacidades.is_empty() {
            return Some(Vec::new());
        }
        let respuesta = self
            .http
            .post(format!(
                "{}/internal/empresas/capacidades",
                self.identity_url
            ))
            .header("content-type", "application/json")
            .header("x-internal-key", &self.clave)
            .json(&json!({ "empresa_ids": empresas, "capacidades": capacidades }))
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo = respuesta.json::<RespuestaCapacidades>().await.ok()?;
        Some(cuerpo.capacidades)
    }

    pub async fn ofertas(&self, ids: &[String]) -> Vec<OfertaExterna> {
        if ids.is_empty() {
            return Vec::new();
        }
        let unicos: Vec<&String> = {
            let mut vistos = std::collections::HashSet::new();
            ids.iter().filter(|id| vistos.insert(*id)).collect()
        };
        let unidos = unicos
            .iter()
            .map(|id| urlencoding::encode(id).into_owned())
            .collect::<Vec<_>>()
            .join(",");
        let Some(respuesta) = self
            .http
            .get(format!("{}/internal/ofertas?ids={unidos}", self.stores_url))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()
        else {
            return Vec::new();
        };
        if !respuesta.status().is_success() {
            return Vec::new();
        }
        respuesta
            .json::<Vec<OfertaExterna>>()
            .await
            .unwrap_or_default()
    }

    pub async fn precios(
        &self,
        empresa_id: &str,
        lineas: Vec<Value>,
        lista_codigo: Option<&str>,
        segmento_codigo: Option<&str>,
    ) -> PreciosExternos {
        if lineas.is_empty() {
            return PreciosExternos::Ok(Vec::new());
        }
        let mut cuerpo = json!({ "empresa_id": empresa_id, "lineas": lineas });
        if let Some(lista) = lista_codigo {
            cuerpo["lista_codigo"] = json!(lista);
        }
        if let Some(segmento) = segmento_codigo {
            cuerpo["segmento_codigo"] = json!(segmento);
        }
        let Some(respuesta) = self
            .http
            .post(format!("{}/internal/items/precios", self.catalog_url))
            .header("content-type", "application/json")
            .header("x-internal-key", &self.clave)
            .json(&cuerpo)
            .send()
            .await
            .ok()
        else {
            return PreciosExternos::Error;
        };
        let status = respuesta.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return PreciosExternos::NoEncontrado;
        }
        if !status.is_success() {
            return PreciosExternos::Error;
        }
        respuesta
            .json::<RespuestaPrecios>()
            .await
            .map(|cuerpo| PreciosExternos::Ok(cuerpo.precios))
            .unwrap_or(PreciosExternos::Error)
    }

    pub async fn cotizacion(
        &self,
        empresa_id: &str,
        item_id: &str,
        desde: &str,
        hasta: &str,
    ) -> Option<Cotizacion> {
        let query = url::form_urlencoded_pair(&[
            ("empresa_id", empresa_id),
            ("desde", desde),
            ("hasta", hasta),
        ]);
        let respuesta = self
            .http
            .get(format!(
                "{}/internal/items/{}/cotizacion?{query}",
                self.catalog_url,
                urlencoding::encode(item_id)
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        respuesta.json::<Cotizacion>().await.ok()
    }

    /// `None` = scheduling no disponible.
    pub async fn disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
        inicia_en: &str,
        termina_en: &str,
    ) -> Option<bool> {
        let respuesta = self
            .http
            .post(format!(
                "{}/internal/scheduling/disponibilidad",
                self.scheduling_url
            ))
            .header("content-type", "application/json")
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "recurso_id": recurso_id,
                "inicia_en": inicia_en,
                "termina_en": termina_en,
            }))
            .send()
            .await
            .ok()?;
        if respuesta.status() == reqwest::StatusCode::CONFLICT {
            return Some(false);
        }
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo = respuesta.json::<RespuestaDisponibilidad>().await.ok()?;
        Some(cuerpo.disponible)
    }

    pub async fn registrar_credito(
        &self,
        empresa_id: &str,
        parte_id: &str,
        order_id: &str,
        monto_cents: i64,
        concepto: &str,
        vendedor_id: Option<&str>,
    ) -> CreditoEstado {
        let mut cuerpo = json!({
            "empresa_id": empresa_id,
            "parte_id": parte_id,
            "order_id": order_id,
            "monto_cents": monto_cents,
            "concepto": concepto,
        });
        if let Some(vendedor) = vendedor_id {
            cuerpo["vendedor_id"] = json!(vendedor);
        }
        let Some(respuesta) = self
            .http
            .post(format!("{}/internal/finance/creditos", self.finance_url))
            .header("content-type", "application/json")
            .header("x-internal-key", &self.clave)
            .json(&cuerpo)
            .send()
            .await
            .ok()
        else {
            return CreditoEstado::Error;
        };
        if respuesta.status() == reqwest::StatusCode::CONFLICT {
            return CreditoEstado::Rechazado;
        }
        if !respuesta.status().is_success() {
            return CreditoEstado::Error;
        }
        CreditoEstado::Ok
    }

    pub async fn anular_credito(&self, order_id: &str) -> bool {
        let Some(respuesta) = self
            .http
            .delete(format!(
                "{}/internal/finance/creditos/orden/{}",
                self.finance_url,
                urlencoding::encode(order_id)
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()
        else {
            return false;
        };
        respuesta.status().is_success()
    }

    pub async fn items_lote(&self, skus: &[String]) -> Option<Vec<ItemStock>> {
        if skus.is_empty() {
            return Some(Vec::new());
        }
        let unidos = skus
            .iter()
            .map(|sku| urlencoding::encode(sku).into_owned())
            .collect::<Vec<_>>()
            .join(",");
        let respuesta = self
            .http
            .get(format!(
                "{}/internal/items/lote?skus={unidos}",
                self.catalog_url
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        respuesta.json::<Vec<ItemStock>>().await.ok()
    }
}

#[derive(Deserialize)]
struct RespuestaCapacidades {
    #[serde(default)]
    capacidades: Vec<Capacidad>,
}

#[derive(Deserialize)]
struct RespuestaPrecios {
    #[serde(default)]
    precios: Vec<PrecioLinea>,
}

#[derive(Deserialize)]
struct RespuestaDisponibilidad {
    #[serde(default)]
    disponible: bool,
}

#[async_trait]
impl ClientesPort for Clientes {
    async fn capacidades(
        &self,
        empresas: &[String],
        capacidades: &[String],
    ) -> Option<Vec<Capacidad>> {
        Clientes::capacidades(self, empresas, capacidades).await
    }
    async fn ofertas(&self, ids: &[String]) -> Vec<OfertaExterna> {
        Clientes::ofertas(self, ids).await
    }
    async fn precios(
        &self,
        empresa_id: &str,
        lineas: Vec<Value>,
        lista_codigo: Option<&str>,
        segmento_codigo: Option<&str>,
    ) -> PreciosExternos {
        Clientes::precios(self, empresa_id, lineas, lista_codigo, segmento_codigo).await
    }
    async fn cotizacion(
        &self,
        empresa_id: &str,
        item_id: &str,
        desde: &str,
        hasta: &str,
    ) -> Option<Cotizacion> {
        Clientes::cotizacion(self, empresa_id, item_id, desde, hasta).await
    }
    async fn disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
        inicia_en: &str,
        termina_en: &str,
    ) -> Option<bool> {
        Clientes::disponibilidad(self, empresa_id, recurso_id, inicia_en, termina_en).await
    }
    async fn registrar_credito(
        &self,
        empresa_id: &str,
        parte_id: &str,
        order_id: &str,
        monto_cents: i64,
        concepto: &str,
        vendedor_id: Option<&str>,
    ) -> CreditoEstado {
        Clientes::registrar_credito(
            self,
            empresa_id,
            parte_id,
            order_id,
            monto_cents,
            concepto,
            vendedor_id,
        )
        .await
    }
    async fn anular_credito(&self, order_id: &str) -> bool {
        Clientes::anular_credito(self, order_id).await
    }
    async fn items_lote(&self, skus: &[String]) -> Option<Vec<ItemStock>> {
        Clientes::items_lote(self, skus).await
    }
}

mod url {
    /// Codifica pares como `application/x-www-form-urlencoded` sin arrastrar
    /// una dependencia extra (los valores son UUID/fechas).
    pub fn form_urlencoded_pair(pares: &[(&str, &str)]) -> String {
        pares
            .iter()
            .map(|(clave, valor)| {
                format!(
                    "{}={}",
                    urlencoding::encode(clave),
                    urlencoding::encode(valor)
                )
            })
            .collect::<Vec<_>>()
            .join("&")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_codificar_los_pares_de_query() {
        assert_eq!(
            url::form_urlencoded_pair(&[("empresa_id", "e 1"), ("desde", "2026-01-01")]),
            "empresa_id=e%201&desde=2026-01-01"
        );
    }
}
