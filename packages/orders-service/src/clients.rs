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
    item_url: String,
    pricing_url: String,
    inventory_url: String,
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
pub struct ConfiguracionOperativa {
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub fulfillment: Vec<String>,
    #[serde(default)]
    pub requiere_agenda: bool,
    #[serde(default)]
    pub trazabilidad: bool,
    #[serde(default = "verdadero")]
    pub requiere_vendedor: bool,
    #[serde(default = "verdadero")]
    pub comision: bool,
}

fn verdadero() -> bool {
    true
}

impl Default for ConfiguracionOperativa {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            fulfillment: Vec::new(),
            requiere_agenda: false,
            trazabilidad: false,
            requiere_vendedor: true,
            comision: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfiguracionExterna {
    Ok(ConfiguracionOperativa),
    SinConfiguracion,
    Error,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct OfertaExterna {
    pub id: String,
    pub tienda_id: String,
    pub vendedor_id: Option<String>,
    pub empresa_id: String,
    #[serde(default)]
    pub negocio_id: Option<String>,
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
pub struct CuponValidado {
    pub cupon_id: String,
    pub codigo: String,
    pub descuento_cents: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CuponExterno {
    Ok(CuponValidado),
    Invalido { codigo: String, mensaje: String },
    NoDisponible,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PromocionLinea {
    pub id: String,
    pub nombre: String,
    pub tipo: String,
    #[serde(default)]
    pub cantidad_requerida: Option<i64>,
    #[serde(default)]
    pub cantidad_gratis: Option<i64>,
    #[serde(default)]
    pub descuento_porcentaje: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PrecioLinea {
    pub clave: String,
    pub origen: String,
    pub precio_unitario_cents: i64,
    #[serde(default)]
    pub precio_total_cents: i64,
    #[serde(default)]
    pub promocion: Option<PromocionLinea>,
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

#[derive(Debug, Deserialize)]
struct ItemV2Stock {
    empresa_id: String,
    sku: String,
    #[serde(default)]
    track_serial: bool,
    estado: String,
}

#[derive(Debug, Deserialize)]
struct StockV2 {
    sku: String,
    #[serde(default)]
    stock: i64,
    #[serde(default)]
    seriales_disponibles: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TasaImpuestoExterna {
    pub codigo: String,
    pub nombre: String,
    pub tasa: f64,
    pub exenta: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfiguracionFiscal {
    pub tasa_iva: f64,
    pub impuesto_activo: bool,
    pub precios_con_impuesto: bool,
    pub moneda: String,
    pub moneda_iso: String,
    pub tasas: Vec<TasaImpuestoExterna>,
    pub tasa_default_codigo: String,
    pub retencion_activa: bool,
    pub tasa_retencion: f64,
    pub retencion_codigo: String,
}

impl ConfiguracionFiscal {
    /// Resuelve la tasa de una línea. `None` significa que el código no existe
    /// en la región y la operación debe fallar cerrada.
    pub fn tasa(&self, codigo: Option<&str>) -> Option<(f64, bool)> {
        let codigo = codigo
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .unwrap_or(self.tasa_default_codigo.as_str());
        if let Some(tasa) = self.tasas.iter().find(|tasa| tasa.codigo == codigo) {
            return Some((tasa.tasa, tasa.exenta));
        }
        if self.tasas.is_empty() {
            return match codigo {
                "GENERAL" => Some((self.tasa_iva, self.tasa_iva == 0.0)),
                "EXENTA" => Some((0.0, true)),
                _ => None,
            };
        }
        None
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct StockUbicacion {
    #[serde(default)]
    pub sku: String,
    #[serde(default)]
    pub stock: i64,
    #[serde(default)]
    pub seriales_disponibles: i64,
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
        negocio_id: Option<&str>,
        capacidades: &[String],
    ) -> Option<Vec<Capacidad>>;

    /// Tasa publicada base→quote; `None` si no hay o finance no responde.
    async fn tipo_cambio(&self, _base: &str, _quote: &str) -> Option<f64> {
        None
    }

    /// Configuración operativa del pack; `SinConfiguracion` = sin restricción.
    async fn configuracion(&self, _empresa_id: &str) -> ConfiguracionExterna {
        ConfiguracionExterna::SinConfiguracion
    }

    /// Configuración fiscal efectiva para calcular IVA por línea.
    async fn configuracion_fiscal(
        &self,
        _empresa_id: &str,
        _negocio_id: Option<&str>,
    ) -> Option<ConfiguracionFiscal> {
        None
    }
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
    async fn stock_ubicacion(
        &self,
        _empresa_id: &str,
        _ubicacion_id: &str,
        _skus: &[String],
    ) -> Option<Vec<StockUbicacion>> {
        None
    }

    async fn cupon_validar(
        &self,
        _empresa_id: &str,
        _codigo: &str,
        _monto_cents: i64,
    ) -> CuponExterno {
        CuponExterno::NoDisponible
    }

    async fn cupon_canjear(
        &self,
        _empresa_id: &str,
        _codigo: &str,
        _order_id: &str,
        _monto_cents: i64,
    ) -> Option<CuponValidado> {
        None
    }

    async fn cupon_liberar(&self, _empresa_id: &str, _codigo: &str, _order_id: &str) {}

    /// Emite una nota de crédito parcial en finance; devuelve su id.
    async fn nota_credito(&self, _payload: &Value) -> Option<String> {
        None
    }

    async fn devolver_stock(
        &self,
        _order_id: &str,
        _devolucion_id: &str,
        _items: &[Value],
    ) -> bool {
        false
    }
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
            item_url: config.item_service_url.clone(),
            pricing_url: config.pricing_service_url.clone(),
            inventory_url: config.inventory_service_url.clone(),
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
        negocio_id: Option<&str>,
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
            .json(&json!({
                "empresa_ids": empresas,
                "capacidades": capacidades,
                "negocio_id": negocio_id,
            }))
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo = respuesta.json::<RespuestaCapacidades>().await.ok()?;
        Some(cuerpo.capacidades)
    }

    pub async fn tipo_cambio(&self, base: &str, quote: &str) -> Option<f64> {
        let respuesta = self
            .http
            .get(format!(
                "{}/internal/finance/tipos-cambio",
                self.finance_url
            ))
            .query(&[("base", base), ("quote", quote)])
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo: Value = respuesta.json().await.ok()?;
        let tasa = cuerpo.get("tasa")?;
        tasa.as_str()
            .and_then(|valor| valor.parse::<f64>().ok())
            .or_else(|| tasa.as_f64())
    }

    pub async fn configuracion_fiscal(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Option<ConfiguracionFiscal> {
        let mut peticion = self
            .http
            .get(format!(
                "{}/internal/finance/configuracion-fiscal",
                self.finance_url
            ))
            .query(&[("empresa_id", empresa_id)])
            .header("x-internal-key", &self.clave);
        if let Some(negocio_id) = negocio_id {
            peticion = peticion.query(&[("negocio_id", negocio_id)]);
        }
        let respuesta = peticion.send().await.ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo: Value = respuesta.json().await.ok()?;
        let tasas = cuerpo
            .get("tasas")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| {
                        Some(TasaImpuestoExterna {
                            codigo: item.get("codigo")?.as_str()?.to_string(),
                            nombre: item
                                .get("nombre")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string(),
                            tasa: item.get("tasa")?.as_f64()?,
                            exenta: item.get("exenta").and_then(Value::as_bool).unwrap_or(false),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        Some(ConfiguracionFiscal {
            tasa_iva: cuerpo.get("tasa_iva").and_then(Value::as_f64)?,
            impuesto_activo: cuerpo
                .get("impuesto_activo")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            precios_con_impuesto: cuerpo
                .get("precios_con_impuesto")
                .and_then(Value::as_bool)
                .unwrap_or(true),
            moneda: cuerpo
                .get("moneda")
                .and_then(Value::as_str)
                .unwrap_or("NIO")
                .to_string(),
            moneda_iso: cuerpo
                .get("moneda_iso")
                .and_then(Value::as_str)
                .unwrap_or("NIO")
                .to_string(),
            tasas,
            tasa_default_codigo: cuerpo
                .get("tasa_default_codigo")
                .and_then(Value::as_str)
                .unwrap_or("GENERAL")
                .to_string(),
            retencion_activa: cuerpo
                .get("retencion_activa")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            tasa_retencion: cuerpo
                .get("tasa_retencion")
                .and_then(Value::as_f64)
                .unwrap_or(0.0),
            retencion_codigo: cuerpo
                .get("retencion_codigo")
                .and_then(Value::as_str)
                .unwrap_or("IR")
                .to_string(),
        })
    }

    pub async fn configuracion(&self, empresa_id: &str) -> ConfiguracionExterna {
        let respuesta = match self
            .http
            .get(format!(
                "{}/internal/empresas/{empresa_id}/configuracion",
                self.identity_url
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
        {
            Ok(respuesta) => respuesta,
            Err(_) => return ConfiguracionExterna::Error,
        };
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return ConfiguracionExterna::SinConfiguracion;
        }
        if !respuesta.status().is_success() {
            return ConfiguracionExterna::Error;
        }
        match respuesta.json::<ConfiguracionOperativa>().await {
            Ok(config) => ConfiguracionExterna::Ok(config),
            Err(_) => ConfiguracionExterna::Error,
        }
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
            .post(format!("{}/internal/items/precios", self.pricing_url))
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
                self.pricing_url,
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
                "{}/internal/v2/items/lote?skus={unidos}",
                self.item_url
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let items = respuesta.json::<Vec<ItemV2Stock>>().await.ok()?;

        let mut empresas: Vec<String> = Vec::new();
        for item in &items {
            if !empresas.iter().any(|empresa| empresa == &item.empresa_id) {
                empresas.push(item.empresa_id.clone());
            }
        }
        let mut stock_por_empresa: Vec<Vec<StockV2>> = Vec::new();
        for empresa in &empresas {
            let codigos: Vec<String> = items
                .iter()
                .filter(|item| &item.empresa_id == empresa)
                .map(|item| item.sku.clone())
                .collect();
            let respuesta = self
                .http
                .post(format!("{}/internal/inventario/stock", self.inventory_url))
                .header("x-internal-key", &self.clave)
                .json(&json!({ "empresa_id": empresa, "skus": codigos }))
                .send()
                .await
                .ok()?;
            if !respuesta.status().is_success() {
                return None;
            }
            stock_por_empresa.push(respuesta.json::<Vec<StockV2>>().await.ok()?);
        }

        let mut salida = Vec::with_capacity(items.len());
        for item in items {
            let indice = empresas
                .iter()
                .position(|empresa| empresa == &item.empresa_id)?;
            let stock = stock_por_empresa
                .get(indice)?
                .iter()
                .find(|linea| linea.sku.eq_ignore_ascii_case(&item.sku));
            salida.push(ItemStock {
                empresa_id: item.empresa_id,
                sku: item.sku,
                stock: stock.map(|linea| linea.stock).unwrap_or(0),
                track_serial: item.track_serial,
                seriales_disponibles: stock.map(|linea| linea.seriales_disponibles).unwrap_or(0),
                estado: item.estado,
            });
        }
        Some(salida)
    }

    pub async fn stock_ubicacion(
        &self,
        empresa_id: &str,
        ubicacion_id: &str,
        skus: &[String],
    ) -> Option<Vec<StockUbicacion>> {
        if skus.is_empty() {
            return Some(Vec::new());
        }
        let respuesta = self
            .http
            .post(format!("{}/internal/inventario/stock", self.inventory_url))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "ubicacion_id": ubicacion_id,
                "skus": skus,
            }))
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        respuesta.json::<Vec<StockUbicacion>>().await.ok()
    }

    pub async fn cupon_validar(
        &self,
        empresa_id: &str,
        codigo: &str,
        monto_cents: i64,
    ) -> CuponExterno {
        let respuesta = match self
            .http
            .post(format!("{}/internal/cupones/validar", self.pricing_url))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "codigo": codigo,
                "monto_cents": monto_cents,
            }))
            .send()
            .await
        {
            Ok(respuesta) => respuesta,
            Err(_) => return CuponExterno::NoDisponible,
        };
        match respuesta.status().as_u16() {
            200..=299 => match respuesta.json::<CuponValidado>().await {
                Ok(cupon) => CuponExterno::Ok(cupon),
                Err(_) => CuponExterno::NoDisponible,
            },
            400..=499 => {
                let cuerpo = respuesta.json::<Value>().await.unwrap_or(Value::Null);
                CuponExterno::Invalido {
                    codigo: cuerpo
                        .get("codigo")
                        .and_then(Value::as_str)
                        .unwrap_or("CUPON_INVALIDO")
                        .to_string(),
                    mensaje: cuerpo
                        .get("mensaje")
                        .and_then(Value::as_str)
                        .unwrap_or("El cupón no es válido.")
                        .to_string(),
                }
            }
            _ => CuponExterno::NoDisponible,
        }
    }

    pub async fn cupon_canjear(
        &self,
        empresa_id: &str,
        codigo: &str,
        order_id: &str,
        monto_cents: i64,
    ) -> Option<CuponValidado> {
        let respuesta = self
            .http
            .post(format!("{}/internal/cupones/canjear", self.pricing_url))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "codigo": codigo,
                "order_id": order_id,
                "monto_cents": monto_cents,
            }))
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        respuesta.json::<CuponValidado>().await.ok()
    }

    pub async fn cupon_liberar(&self, empresa_id: &str, codigo: &str, order_id: &str) {
        let _ = self
            .http
            .post(format!("{}/internal/cupones/liberar", self.pricing_url))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "empresa_id": empresa_id,
                "codigo": codigo,
                "order_id": order_id,
            }))
            .send()
            .await;
    }

    pub async fn devolver_stock(
        &self,
        order_id: &str,
        devolucion_id: &str,
        items: &[Value],
    ) -> bool {
        self.http
            .post(format!(
                "{}/internal/inventario/devolucion",
                self.inventory_url
            ))
            .header("x-internal-key", &self.clave)
            .json(&json!({
                "order_id": order_id,
                "devolucion_id": devolucion_id,
                "items": items,
            }))
            .send()
            .await
            .map(|respuesta| respuesta.status().is_success())
            .unwrap_or(false)
    }

    pub async fn nota_credito(&self, payload: &Value) -> Option<String> {
        let respuesta = self
            .http
            .post(format!(
                "{}/internal/finance/notas-credito",
                self.finance_url
            ))
            .header("x-internal-key", &self.clave)
            .json(payload)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let cuerpo: Value = respuesta.json().await.ok()?;
        cuerpo
            .get("comprobante_id")
            .or_else(|| cuerpo.get("id"))
            .and_then(Value::as_str)
            .map(str::to_string)
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
        negocio_id: Option<&str>,
        capacidades: &[String],
    ) -> Option<Vec<Capacidad>> {
        Clientes::capacidades(self, empresas, negocio_id, capacidades).await
    }

    async fn tipo_cambio(&self, base: &str, quote: &str) -> Option<f64> {
        Clientes::tipo_cambio(self, base, quote).await
    }

    async fn configuracion_fiscal(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Option<ConfiguracionFiscal> {
        Clientes::configuracion_fiscal(self, empresa_id, negocio_id).await
    }

    async fn configuracion(&self, empresa_id: &str) -> ConfiguracionExterna {
        Clientes::configuracion(self, empresa_id).await
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

    async fn stock_ubicacion(
        &self,
        empresa_id: &str,
        ubicacion_id: &str,
        skus: &[String],
    ) -> Option<Vec<StockUbicacion>> {
        Clientes::stock_ubicacion(self, empresa_id, ubicacion_id, skus).await
    }

    async fn cupon_validar(
        &self,
        empresa_id: &str,
        codigo: &str,
        monto_cents: i64,
    ) -> CuponExterno {
        Clientes::cupon_validar(self, empresa_id, codigo, monto_cents).await
    }

    async fn cupon_canjear(
        &self,
        empresa_id: &str,
        codigo: &str,
        order_id: &str,
        monto_cents: i64,
    ) -> Option<CuponValidado> {
        Clientes::cupon_canjear(self, empresa_id, codigo, order_id, monto_cents).await
    }

    async fn cupon_liberar(&self, empresa_id: &str, codigo: &str, order_id: &str) {
        Clientes::cupon_liberar(self, empresa_id, codigo, order_id).await
    }

    async fn nota_credito(&self, payload: &Value) -> Option<String> {
        Clientes::nota_credito(self, payload).await
    }

    async fn devolver_stock(&self, order_id: &str, devolucion_id: &str, items: &[Value]) -> bool {
        Clientes::devolver_stock(self, order_id, devolucion_id, items).await
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

    #[tokio::test]
    async fn debe_solicitar_la_nota_de_credito_al_servicio_de_finanzas() {
        use axum::routing::post;
        use axum::{Json, Router};
        let app = Router::new().route(
            "/internal/finance/notas-credito",
            post(|| async {
                Json(serde_json::json!({
                    "id": "77777777-7777-4777-8777-777777777777"
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let direccion = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let config = Config::from_pairs([
            (
                "FINANCE_SERVICE_URL",
                format!("http://{direccion}").as_str(),
            ),
            ("INTERNAL_API_KEY", "clave"),
            ("JWT_SECRET", "secreto"),
        ])
        .expect("config");
        let clientes = Clientes::nueva(&config);
        let id = clientes
            .nota_credito(&serde_json::json!({ "orden_id": "o-1" }))
            .await;
        assert_eq!(id.as_deref(), Some("77777777-7777-4777-8777-777777777777"));
    }

    #[test]
    fn debe_mantener_el_marketplace_por_defecto_en_la_configuracion() {
        assert!(verdadero());
        let config = ConfiguracionOperativa::default();
        assert!(config.requiere_vendedor);
        assert!(config.comision);
    }

    #[test]
    fn debe_resolver_tasas_por_codigo_y_fallar_cerrado() {
        let config = ConfiguracionFiscal {
            tasa_iva: 0.15,
            impuesto_activo: true,
            precios_con_impuesto: true,
            moneda: "C$".into(),
            moneda_iso: "NIO".into(),
            tasas: vec![
                TasaImpuestoExterna {
                    codigo: "GENERAL".into(),
                    nombre: "General".into(),
                    tasa: 0.15,
                    exenta: false,
                },
                TasaImpuestoExterna {
                    codigo: "EXENTA".into(),
                    nombre: "Exento".into(),
                    tasa: 0.0,
                    exenta: true,
                },
            ],
            tasa_default_codigo: "GENERAL".into(),
            retencion_activa: true,
            tasa_retencion: 0.02,
            retencion_codigo: "IR".into(),
        };
        assert_eq!(config.tasa(None), Some((0.15, false)));
        assert_eq!(config.tasa(Some("EXENTA")), Some((0.0, true)));
        assert_eq!(config.tasa(Some("NO_EXISTE")), None);

        let legacy = ConfiguracionFiscal {
            tasas: Vec::new(),
            ..config
        };
        assert_eq!(legacy.tasa(Some("GENERAL")), Some((0.15, false)));
        assert_eq!(legacy.tasa(Some("EXENTA")), Some((0.0, true)));
    }
}
