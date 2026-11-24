use super::prelude::*;
use async_trait::async_trait;
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CargoExterno {
    pub referencia: String,
    /// URL de pago alojada cuando la pasarela la devuelve (Tilopay/Pagadito).
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoExterno {
    Aprobado,
    Rechazado,
    Pendiente,
}

/// ACL de la pasarela de pagos (R-AR-6). El dominio solo conoce esta interfaz;
/// los adaptadores concretos se seleccionan por configuración.
#[async_trait]
pub trait ProveedorPagos: Send + Sync {
    fn nombre(&self) -> String;
    async fn crear_cargo(
        &self,
        order_id: &str,
        monto_cents: i64,
        moneda: &str,
    ) -> Result<CargoExterno, ApiError>;
    async fn reembolsar(&self, referencia: &str, monto_cents: i64) -> Result<(), ApiError>;
    async fn contracargar(
        &self,
        referencia: &str,
        monto_cents: i64,
        motivo: &str,
    ) -> Result<(), ApiError>;
    /// Consulta el estado real en la pasarela. El webhook solo es una señal;
    /// la decisión se toma con esta llamada.
    async fn consultar(
        &self,
        _referencia: &str,
        _datos: Option<&Value>,
    ) -> Result<EstadoExterno, ApiError> {
        Ok(EstadoExterno::Pendiente)
    }
}

pub struct ProveedorSimulado;

#[async_trait]
impl ProveedorPagos for ProveedorSimulado {
    fn nombre(&self) -> String {
        "simulado".to_string()
    }

    async fn crear_cargo(
        &self,
        _order_id: &str,
        _monto_cents: i64,
        _moneda: &str,
    ) -> Result<CargoExterno, ApiError> {
        Ok(CargoExterno {
            referencia: format!("sim-{}", Uuid::new_v4()),
            url: None,
        })
    }

    async fn reembolsar(&self, _referencia: &str, _monto_cents: i64) -> Result<(), ApiError> {
        Ok(())
    }

    async fn contracargar(
        &self,
        _referencia: &str,
        _monto_cents: i64,
        _motivo: &str,
    ) -> Result<(), ApiError> {
        Ok(())
    }
}

/// Proveedor manual: no crea cargos ni reembolsa por API. Se usa para
/// efectivo/transferencia o cuando el negocio concilia fuera del sistema.
pub struct ProveedorManual;

#[async_trait]
impl ProveedorPagos for ProveedorManual {
    fn nombre(&self) -> String {
        "manual".to_string()
    }

    async fn crear_cargo(
        &self,
        _order_id: &str,
        _monto_cents: i64,
        _moneda: &str,
    ) -> Result<CargoExterno, ApiError> {
        Err(ApiError::dominio(
            "PROVEEDOR_MANUAL",
            "El proveedor manual no crea cargos externos.",
        ))
    }

    async fn reembolsar(&self, _referencia: &str, _monto_cents: i64) -> Result<(), ApiError> {
        Err(ApiError::dominio(
            "PROVEEDOR_MANUAL",
            "El proveedor manual no procesa reembolsos por API.",
        ))
    }

    async fn contracargar(
        &self,
        _referencia: &str,
        _monto_cents: i64,
        _motivo: &str,
    ) -> Result<(), ApiError> {
        Err(ApiError::dominio(
            "PROVEEDOR_MANUAL",
            "El proveedor manual no procesa contracargos por API.",
        ))
    }
}

/// Adaptador HTTP genérico para una pasarela que exponga `/cargos`,
/// `/reembolsos` y `/contracargos` con `Authorization: Bearer`.
pub struct ProveedorHttp {
    http: Client,
    url: String,
    clave: String,
}

impl ProveedorHttp {
    pub fn nuevo(url: String, clave: Option<String>) -> Self {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            http,
            url: url.trim_end_matches('/').to_string(),
            clave: clave.unwrap_or_default(),
        }
    }

    fn peticion(&self, ruta: &str) -> reqwest::RequestBuilder {
        self.http
            .post(format!("{}/{}", self.url, ruta))
            .bearer_auth(&self.clave)
    }
}

#[async_trait]
impl ProveedorPagos for ProveedorHttp {
    fn nombre(&self) -> String {
        "http".to_string()
    }

    async fn crear_cargo(
        &self,
        order_id: &str,
        monto_cents: i64,
        moneda: &str,
    ) -> Result<CargoExterno, ApiError> {
        let respuesta = self
            .peticion("cargos")
            .json(&json!({
                "orden_id": order_id,
                "monto_cents": monto_cents,
                "moneda": moneda,
            }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("pasarela de pagos"))?;
        if !respuesta.status().is_success() {
            return Err(ApiError::nueva(
                "PAGO_RECHAZADO",
                "La pasarela rechazo la creacion del cargo.",
                axum::http::StatusCode::BAD_GATEWAY,
            ));
        }
        let valor: Value = respuesta.json().await.map_err(|_| ApiError::interno())?;
        let referencia = valor
            .get("referencia")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty())
            .ok_or_else(ApiError::interno)?
            .to_string();
        Ok(CargoExterno {
            referencia,
            url: None,
        })
    }

    async fn reembolsar(&self, referencia: &str, monto_cents: i64) -> Result<(), ApiError> {
        let respuesta = self
            .peticion("reembolsos")
            .json(&json!({ "referencia": referencia, "monto_cents": monto_cents }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("pasarela de pagos"))?;
        if !respuesta.status().is_success() {
            return Err(ApiError::nueva(
                "REEMBOLSO_RECHAZADO",
                "La pasarela rechazo el reembolso.",
                axum::http::StatusCode::BAD_GATEWAY,
            ));
        }
        Ok(())
    }

    async fn contracargar(
        &self,
        referencia: &str,
        monto_cents: i64,
        motivo: &str,
    ) -> Result<(), ApiError> {
        let respuesta = self
            .peticion("contracargos")
            .json(&json!({
                "referencia": referencia,
                "monto_cents": monto_cents,
                "motivo": motivo,
            }))
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("pasarela de pagos"))?;
        if !respuesta.status().is_success() {
            return Err(ApiError::nueva(
                "CONTRACARGO_RECHAZADO",
                "La pasarela rechazo el contracargo.",
                axum::http::StatusCode::BAD_GATEWAY,
            ));
        }
        Ok(())
    }
}
