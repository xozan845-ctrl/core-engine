//! Comprobantes fiscales DGI (cap. 4.4): series con prefijo, secuenciales
//! consecutivos y RUC del emisor. Ley 842: facturación en córdobas (C$).
//! Ley 822: IVA del 15 % se traslada al comprador.

mod consultas;
mod emision;
mod mantenimiento;
mod series;

mod prelude {
    pub(crate) use crate::contabilidad::{Contabilidad, DetalleAsiento, ParametrosAsiento};
    pub(crate) use crate::db::{jsonb, timestamp, uuid_texto, Db};
    pub(crate) use crate::errors::{error_db, ApiError};
    pub(crate) use crate::money;
    pub(crate) use crate::outbox::{Outbox, COMPROBANTE_EMITIDO};
    pub(crate) use chrono::{DateTime, Utc};
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::{json, Map, Value};
    pub(crate) use sqlx::Row;
    pub(crate) use std::sync::Arc;
    pub(crate) use std::time::Duration;
}
use self::prelude::*;
use async_trait::async_trait;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comprobante {
    pub id: String,
    pub serie: String,
    pub numero: String,
    pub tipo: String,
    pub orden_id: Option<String>,
    pub empresa_id: Option<String>,
    pub parte_id: Option<String>,
    pub negocio_id: Option<String>,
    pub base_gravada_cents: i64,
    pub iva_cents: i64,
    pub exento_cents: i64,
    pub retencion_cents: i64,
    pub retencion_tasa_bps: i64,
    pub total_cents: i64,
    pub moneda: String,
    pub estado: String,
    pub datos_cliente: Option<Value>,
    pub emitido_en: Option<String>,
    pub creado_en: String,
    pub electronica_estado: Option<String>,
    pub electronica_referencia: Option<String>,
    pub electronica_enviada_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerieComprobante {
    pub serie: String,
    pub empresa_id: Option<String>,
    pub negocio_id: Option<String>,
    pub tipo: String,
    pub prefijo: String,
    pub secuencial_actual: i64,
    pub ruc_emisor: String,
    pub jurisdiccion: String,
    pub activa: bool,
}

/// Consulta de la orden a orders-service (lectura interna).
#[async_trait]
pub trait Ordenes: Send + Sync {
    async fn consultar(&self, order_id: &str) -> Option<OrdenResumen>;
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OrdenItemFiscal {
    #[serde(default)]
    pub sku: String,
    #[serde(default)]
    pub precio_unitario_cents: i64,
    #[serde(default)]
    pub cantidad: i64,
    #[serde(default)]
    pub descuento_cents: i64,
    #[serde(default)]
    pub impuesto_cents: i64,
    #[serde(default)]
    pub tasa_impuesto_bps: i64,
    #[serde(default)]
    pub impuesto_codigo: Option<String>,
    #[serde(default)]
    pub total_linea_cents: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct OrdenResumen {
    pub total_cents: i64,
    #[serde(default)]
    pub empresa_id: Option<String>,
    #[serde(default)]
    pub negocio_id: Option<String>,
    #[serde(default)]
    pub items: Vec<OrdenItemFiscal>,
    #[serde(default)]
    pub descuento_cupon_cents: i64,
}

pub struct OrdenesHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl OrdenesHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

#[async_trait]
impl Ordenes for OrdenesHttp {
    async fn consultar(&self, order_id: &str) -> Option<OrdenResumen> {
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .ok()?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/orders/{}",
                self.url,
                urlencoding::encode(order_id)
            ))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let valor: Value = respuesta.json().await.ok()?;
        let mut orden: OrdenResumen = serde_json::from_value(valor.clone()).ok()?;
        if orden.empresa_id.is_none() {
            orden.empresa_id = valor
                .get("items")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(|item| item.get("empresa_id"))
                .and_then(Value::as_str)
                .map(str::to_string);
        }
        Some(orden)
    }
}

pub struct SinOrdenes;

#[async_trait]
impl Ordenes for SinOrdenes {
    async fn consultar(&self, _order_id: &str) -> Option<OrdenResumen> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenciaElectronica {
    pub referencia: String,
    pub estado: String,
}

/// ACL para facturación electrónica. El dominio no conoce DGI ni el proveedor;
/// un adaptador HTTP o el simulado se eligen por configuración del tenant.
#[async_trait]
pub trait ProveedorFacturacionElectronica: Send + Sync {
    async fn emitir(
        &self,
        comprobante: &Comprobante,
        ambiente: &str,
    ) -> Result<ReferenciaElectronica, ApiError>;
}

pub struct FacturacionSimulada;

#[async_trait]
impl ProveedorFacturacionElectronica for FacturacionSimulada {
    async fn emitir(
        &self,
        _comprobante: &Comprobante,
        _ambiente: &str,
    ) -> Result<ReferenciaElectronica, ApiError> {
        Ok(ReferenciaElectronica {
            referencia: format!("sim-{}", uuid::Uuid::new_v4()),
            // Un simulador no es un comprobante fiscal válido. Marcarlo como
            // "aceptada" haría creer que la DGI lo autorizó.
            estado: "no_fiscal".to_string(),
        })
    }
}

pub struct FacturacionHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl FacturacionHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms.max(1)),
        }
    }
}

#[async_trait]
impl ProveedorFacturacionElectronica for FacturacionHttp {
    async fn emitir(
        &self,
        comprobante: &Comprobante,
        ambiente: &str,
    ) -> Result<ReferenciaElectronica, ApiError> {
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| ApiError::interno())?;
        let respuesta = cliente
            .post(format!("{}/comprobantes", self.url))
            .bearer_auth(&self.clave)
            .json(&json!({ "comprobante": comprobante, "ambiente": ambiente }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("facturacion-electronica"))?;
        if !respuesta.status().is_success() {
            return Err(ApiError::nueva(
                "FACTURACION_RECHAZADA",
                "El proveedor rechazo el comprobante.",
                axum::http::StatusCode::BAD_GATEWAY,
            ));
        }
        let valor: Value = respuesta.json().await.map_err(|_| ApiError::interno())?;
        Ok(ReferenciaElectronica {
            referencia: valor
                .get("referencia")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            estado: valor
                .get("estado")
                .and_then(Value::as_str)
                .unwrap_or("aceptada")
                .to_string(),
        })
    }
}

/// Adaptador de referencia para un proveedor electrónico nacional (DGI y
/// similares). Genera el XML fiscal y lo publica en un gateway de certificación
/// configurado por entorno; la firma/certificado y el endpoint los provee el
/// país. No contiene claves ni lógica específica hardcodeada.
pub struct FacturacionDgi {
    url: String,
    clave: String,
    timeout: Duration,
}

impl FacturacionDgi {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url: url.trim_end_matches('/').to_string(),
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms.max(1)),
        }
    }

    fn xml(comprobante: &Comprobante, ambiente: &str) -> String {
        let datos = comprobante.datos_cliente.as_ref();
        let cliente_ruc = datos
            .and_then(|valor| valor.get("ruc"))
            .and_then(Value::as_str)
            .unwrap_or("-");
        let cliente_nombre = datos
            .and_then(|valor| valor.get("razon_social"))
            .and_then(Value::as_str)
            .unwrap_or("-");
        let emisor_ruc = datos
            .and_then(|valor| valor.get("emisor_ruc"))
            .and_then(Value::as_str)
            .unwrap_or("-");
        let items = datos
            .and_then(|valor| valor.get("items"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|item| {
                format!(
                    "<Item><SKU>{}</SKU><Cantidad>{}</Cantidad><Base>{}</Base><IVA>{}</IVA><Total>{}</Total></Item>",
                    xml_escape(item.get("sku").and_then(Value::as_str).unwrap_or("-")),
                    item.get("cantidad").and_then(Value::as_i64).unwrap_or(0),
                    item.get("base_cents").and_then(Value::as_i64).unwrap_or(0),
                    item.get("iva_cents").and_then(Value::as_i64).unwrap_or(0),
                    item.get("total_cents").and_then(Value::as_i64).unwrap_or(0),
                )
            })
            .collect::<String>();
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
             <ComprobanteElectronico>\
             <Ambiente>{}</Ambiente>\
             <Emisor><RUC>{}</RUC></Emisor>\
             <Receptor><RUC>{}</RUC><RazonSocial>{}</RazonSocial></Receptor>\
             <Serie>{}</Serie><Numero>{}</Numero><Tipo>{}</Tipo>\
             <FechaEmision>{}</FechaEmision><Moneda>{}</Moneda>\
             <Totales><BaseGravada>{}</BaseGravada><IVA>{}</IVA><Exento>{}</Exento>\
             <Retencion>{}</Retencion><Total>{}</Total></Totales>\
             <Items>{}</Items>\
             </ComprobanteElectronico>",
            xml_escape(ambiente),
            xml_escape(emisor_ruc),
            xml_escape(cliente_ruc),
            xml_escape(cliente_nombre),
            xml_escape(&comprobante.serie),
            xml_escape(&comprobante.numero),
            xml_escape(&comprobante.tipo),
            xml_escape(comprobante.emitido_en.as_deref().unwrap_or("-")),
            xml_escape(&comprobante.moneda),
            comprobante.base_gravada_cents,
            comprobante.iva_cents,
            comprobante.exento_cents,
            comprobante.retencion_cents,
            comprobante.total_cents,
            items,
        )
    }
}

fn xml_escape(valor: &str) -> String {
    valor
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[async_trait]
impl ProveedorFacturacionElectronica for FacturacionDgi {
    async fn emitir(
        &self,
        comprobante: &Comprobante,
        ambiente: &str,
    ) -> Result<ReferenciaElectronica, ApiError> {
        if self.url.is_empty() {
            return Err(ApiError::dominio(
                "PROVEEDOR_NO_CONFIGURADO",
                "No hay gateway DGI configurado (FACTURACION_PROVEEDOR_URL).",
            ));
        }
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| ApiError::interno())?;
        let respuesta = cliente
            .post(format!("{}/comprobantes", self.url))
            .bearer_auth(&self.clave)
            .header("content-type", "application/xml")
            .body(Self::xml(comprobante, ambiente))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("facturacion-electronica"))?;
        if !respuesta.status().is_success() {
            return Err(ApiError::nueva(
                "FACTURACION_RECHAZADA",
                "El gateway DGI rechazo el comprobante.",
                axum::http::StatusCode::BAD_GATEWAY,
            ));
        }
        let valor: Value = respuesta.json().await.map_err(|_| ApiError::interno())?;
        Ok(ReferenciaElectronica {
            referencia: valor
                .get("referencia")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            estado: valor
                .get("estado")
                .and_then(Value::as_str)
                .unwrap_or("aceptada")
                .to_string(),
        })
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EmitirComprobante {
    pub tipo: String,
    pub orden_id: String,
    #[serde(default)]
    pub parte_id: Option<String>,
    #[serde(default)]
    pub razon_social: Option<String>,
    #[serde(default)]
    pub ruc: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct NotaCreditoDto {
    pub orden_id: String,
    pub devolucion_id: String,
    pub empresa_id: String,
    #[serde(default)]
    pub negocio_id: Option<String>,
    #[serde(default)]
    pub parte_id: Option<String>,
    pub base_cents: i64,
    pub iva_cents: i64,
    pub total_cents: i64,
    #[serde(default)]
    pub moneda: Option<String>,
    #[serde(default)]
    pub items: Value,
}

pub struct Facturacion {
    db: Arc<Db>,
    outbox: Outbox,
    contabilidad: Arc<Contabilidad>,
    ordenes: Arc<dyn Ordenes>,
    emitir: bool,
    http: Option<Arc<FacturacionHttp>>,
    dgi: Option<Arc<FacturacionDgi>>,
    pdf_almacen: Option<Arc<crate::pdf_almacen::PdfAlmacen>>,
    /// El proveedor configurado fue certificado por el país/proveedor.
    proveedor_certificado: bool,
    /// Aceptación explícita de operar el piloto sin comprobante fiscal.
    permitir_sin_certificado: bool,
}

impl Facturacion {
    pub fn nueva(
        db: Arc<Db>,
        outbox: Outbox,
        contabilidad: Arc<Contabilidad>,
        ordenes: Arc<dyn Ordenes>,
        emitir: bool,
    ) -> Self {
        Self {
            db,
            outbox,
            contabilidad,
            ordenes,
            emitir,
            http: None,
            dgi: None,
            pdf_almacen: None,
            proveedor_certificado: false,
            // Por defecto el constructor de bajo nivel permite simular para
            // tests; `main` lo sobrescribe con la configuración real.
            permitir_sin_certificado: true,
        }
    }

    pub fn con_certificacion(
        mut self,
        proveedor_certificado: bool,
        permitir_sin_certificado: bool,
    ) -> Self {
        self.proveedor_certificado = proveedor_certificado;
        self.permitir_sin_certificado = permitir_sin_certificado;
        self
    }

    pub fn con_pdf_almacen(mut self, almacen: crate::pdf_almacen::PdfAlmacen) -> Self {
        self.pdf_almacen = Some(Arc::new(almacen));
        self
    }

    pub fn con_proveedor_http(
        mut self,
        url: String,
        clave: Option<String>,
        timeout_ms: u64,
    ) -> Self {
        self.http = Some(Arc::new(FacturacionHttp::nueva(url, clave, timeout_ms)));
        self
    }

    pub fn con_proveedor_dgi(
        mut self,
        url: String,
        clave: Option<String>,
        timeout_ms: u64,
    ) -> Self {
        self.dgi = Some(Arc::new(FacturacionDgi::nueva(url, clave, timeout_ms)));
        self
    }
}

fn comprobante_de_fila(fila: &sqlx::postgres::PgRow) -> Comprobante {
    Comprobante {
        id: uuid_texto(fila, "id").unwrap_or_default(),
        serie: fila.try_get("serie").unwrap_or_default(),
        numero: fila.try_get("numero").unwrap_or_default(),
        tipo: fila.try_get("tipo").unwrap_or_default(),
        orden_id: uuid_texto(fila, "orden_id").ok(),
        empresa_id: uuid_texto(fila, "empresa_id").ok(),
        negocio_id: uuid_texto(fila, "negocio_id").ok(),
        parte_id: uuid_texto(fila, "parte_id").ok(),
        base_gravada_cents: fila.try_get::<i32, _>("base_gravada_cents").unwrap_or(0) as i64,
        iva_cents: fila.try_get::<i32, _>("iva_cents").unwrap_or(0) as i64,
        exento_cents: fila.try_get::<i32, _>("exento_cents").unwrap_or(0) as i64,
        retencion_cents: fila.try_get::<i32, _>("retencion_cents").unwrap_or(0) as i64,
        retencion_tasa_bps: fila.try_get::<i32, _>("retencion_tasa_bps").unwrap_or(0) as i64,
        total_cents: fila.try_get::<i32, _>("total_cents").unwrap_or(0) as i64,
        moneda: fila.try_get("moneda").unwrap_or_default(),
        estado: fila.try_get("estado").unwrap_or_default(),
        datos_cliente: jsonb(fila, "datos_cliente").ok(),
        emitido_en: fila
            .try_get::<Option<DateTime<Utc>>, _>("emitido_en")
            .ok()
            .flatten()
            .map(crate::db::iso_ms),
        creado_en: timestamp(fila, "creado_en").unwrap_or_default(),
        electronica_estado: fila.try_get("electronica_estado").ok(),
        electronica_referencia: fila.try_get("electronica_referencia").ok(),
        electronica_enviada_en: fila
            .try_get::<Option<DateTime<Utc>>, _>("electronica_enviada_en")
            .ok()
            .flatten()
            .map(crate::db::iso_ms),
    }
}

fn serie_de_fila(fila: &sqlx::postgres::PgRow) -> SerieComprobante {
    SerieComprobante {
        serie: fila.try_get("serie").unwrap_or_default(),
        empresa_id: uuid_texto(fila, "empresa_id").ok(),
        negocio_id: uuid_texto(fila, "negocio_id").ok(),
        tipo: fila.try_get("tipo").unwrap_or_default(),
        prefijo: fila.try_get("prefijo").unwrap_or_default(),
        secuencial_actual: fila.try_get::<i32, _>("secuencial_actual").unwrap_or(0) as i64,
        ruc_emisor: fila.try_get("ruc_emisor").unwrap_or_default(),
        jurisdiccion: fila.try_get("jurisdiccion").unwrap_or_default(),
        activa: fila.try_get("activa").unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct OrdenesFijas;

    #[async_trait]
    impl Ordenes for OrdenesFijas {
        async fn consultar(&self, order_id: &str) -> Option<OrdenResumen> {
            (order_id == "orden-1").then_some(OrdenResumen {
                total_cents: 100_000,
                empresa_id: None,
                negocio_id: None,
                items: Vec::new(),
                descuento_cupon_cents: 0,
            })
        }
    }

    #[tokio::test]
    async fn debe_consultar_la_orden_por_la_superficie_interna() {
        let ordenes = OrdenesFijas;
        assert_eq!(
            ordenes
                .consultar("orden-1")
                .await
                .map(|orden| orden.total_cents),
            Some(100_000)
        );
        assert!(ordenes.consultar("otra").await.is_none());
    }

    #[test]
    fn debe_calcular_la_base_y_el_iva_del_comprobante() {
        let base = money::desde_centavos(100_000).expect("base");
        let iva = money::multiplicar_por(base, 0.15).expect("iva");
        assert_eq!(iva, 15_000);
        assert_eq!(money::sumar(base, iva).expect("total"), 115_000);
        assert_eq!(money::string(115_000), "1150.00");
    }

    #[test]
    fn debe_generar_el_xml_del_adaptador_dgi() {
        let comprobante = Comprobante {
            id: "c-1".into(),
            serie: "F001".into(),
            numero: "F001-00000001".into(),
            tipo: "FACTURA".into(),
            orden_id: Some("o-1".into()),
            empresa_id: None,
            parte_id: None,
            negocio_id: None,
            base_gravada_cents: 100_000,
            iva_cents: 15_000,
            exento_cents: 5_000,
            retencion_cents: 2_000,
            retencion_tasa_bps: 200,
            total_cents: 120_000,
            moneda: "C$".into(),
            estado: "EMITIDO".into(),
            datos_cliente: Some(json!({
                "ruc": "RUC-CLIENTE",
                "razon_social": "Cliente & Cia",
                "emisor_ruc": "RUC-EMISOR",
                "items": [{"sku": "SKU<1>", "cantidad": 1, "base_cents": 100_000,
                           "iva_cents": 15_000, "total_cents": 120_000}]
            })),
            emitido_en: Some("2026-01-01T00:00:00.000Z".into()),
            creado_en: "2026-01-01T00:00:00.000Z".into(),
            electronica_estado: None,
            electronica_referencia: None,
            electronica_enviada_en: None,
        };
        let xml = FacturacionDgi::xml(&comprobante, "pruebas");
        assert!(xml.contains("<ComprobanteElectronico>"));
        assert!(xml.contains("<RUC>RUC-EMISOR</RUC>"));
        assert!(xml.contains("Cliente &amp; Cia"));
        assert!(xml.contains("SKU&lt;1&gt;"));
        assert!(xml.contains("<Retencion>2000</Retencion>"));
    }
}
