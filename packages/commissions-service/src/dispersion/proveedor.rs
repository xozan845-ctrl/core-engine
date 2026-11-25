//! Adaptadores de dispersión (Tilopay/Pagadito).

use super::{CuentaPago, Payout, PROVEEDORES_DISPERSION};
use crate::errors::ApiError;
use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

/// Adaptador de dispersión. La plataforma puede elegir Tilopay o Pagadito.
#[async_trait]
pub trait ProveedorDispersion: Send + Sync {
    fn nombre(&self) -> String;
    /// Devuelve la referencia externa que asigna el proveedor.
    async fn enviar(&self, payout: &Payout, cuenta: &CuentaPago) -> Result<String, ApiError>;
    async fn consultar(&self, _referencia: &str) -> Result<String, ApiError> {
        Ok("pendiente".to_string())
    }
}

/// Adaptador HTTP configurable para las APIs de dispersión.
///
/// Las rutas exactas cambian por proveedor y por contrato comercial, así que
/// se configuran en el entorno (`TILOPAY_DISPERSION_URL`/`_PATH` y
/// `PAGADITO_DISPERSION_URL`/`_PATH`). Sin URL el adaptador falla cerrado: no
/// se puede enviar dinero a una ruta adivinada.
pub struct DispersionHttp {
    proveedor: String,
    url: Option<String>,
    token: String,
    http: Client,
}

impl DispersionHttp {
    pub fn nuevo(proveedor: &str) -> Result<Self, ApiError> {
        let proveedor = proveedor.trim().to_ascii_lowercase();
        if !PROVEEDORES_DISPERSION.contains(&proveedor.as_str()) {
            return Err(ApiError::dominio(
                "DISPERSION_PROVEEDOR_INVALIDO",
                "El proveedor de dispersión debe ser tilopay o pagadito.",
            ));
        }
        let credenciales = crate::config::credenciales_dispersion(&proveedor);
        Ok(Self {
            proveedor,
            url: credenciales.url,
            token: credenciales.token,
            http: Client::builder()
                .timeout(Duration::from_secs(20))
                .build()
                .unwrap_or_else(|_| Client::new()),
        })
    }
}

#[async_trait]
impl ProveedorDispersion for DispersionHttp {
    fn nombre(&self) -> String {
        self.proveedor.clone()
    }

    async fn enviar(&self, payout: &Payout, cuenta: &CuentaPago) -> Result<String, ApiError> {
        let Some(url) = &self.url else {
            return Err(ApiError::dominio(
                "DISPERSION_NO_CONFIGURADA",
                format!(
                    "Falta {}_DISPERSION_URL para pagar con {}.",
                    self.proveedor.to_ascii_uppercase(),
                    self.proveedor
                ),
            ));
        };
        if cuenta.referencia_externa.trim().is_empty() {
            return Err(ApiError::dominio(
                "CUENTA_PAGO_INCOMPLETA",
                "La cuenta de destino no tiene referencia externa.",
            ));
        }
        let mut peticion = self.http.post(url).json(&json!({
            "referencia": payout.id,
            "fecha": payout.fecha,
            "beneficiario": {
                "tipo": payout.beneficiario_tipo,
                "id": payout.beneficiario_id,
                "cuenta": cuenta.referencia_externa,
            },
            "monto": payout.monto_neto_cents as f64 / 100.0,
            "moneda": payout.moneda,
            "concepto": "Liquidacion diaria de marketplace",
        }));
        if !self.token.is_empty() {
            peticion = peticion.bearer_auth(&self.token);
        }
        let respuesta = peticion
            .send()
            .await
            .map_err(|_| ApiError::servicio_no_disponible("pasarela de dispersión"))?;
        let estado = respuesta.status();
        let valor: Value = respuesta.json().await.unwrap_or(Value::Null);
        if !estado.is_success() {
            return Err(ApiError::dominio(
                "DISPERSION_RECHAZADA",
                valor
                    .get("message")
                    .or_else(|| valor.get("error"))
                    .and_then(Value::as_str)
                    .unwrap_or("La pasarela rechazó la dispersión.")
                    .to_string(),
            ));
        }
        Ok(valor
            .get("referencia")
            .or_else(|| valor.get("id"))
            .or_else(|| valor.get("transaction_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .unwrap_or(&payout.id)
            .to_string())
    }
}

pub struct DispersionNoDisponible;

#[async_trait]
impl ProveedorDispersion for DispersionNoDisponible {
    fn nombre(&self) -> String {
        "no-disponible".to_string()
    }

    async fn enviar(&self, _payout: &Payout, _cuenta: &CuentaPago) -> Result<String, ApiError> {
        Err(ApiError::servicio_no_disponible("pasarela de dispersión"))
    }
}
