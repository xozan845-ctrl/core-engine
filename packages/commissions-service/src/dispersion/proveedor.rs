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
#[derive(Debug)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dispersion::{CuentaPago, Payout};
    use axum::extract::Json as AxumJson;
    use axum::routing::post;
    use axum::Router;

    fn payout() -> Payout {
        Payout {
            id: "00000000-0000-0000-0000-000000000001".into(),
            fecha: "2026-11-25".into(),
            beneficiario_tipo: "vendedor".into(),
            beneficiario_id: "00000000-0000-0000-0000-000000000002".into(),
            empresa_id: None,
            negocio_id: None,
            monto_cents: 1_000,
            retencion_cents: 0,
            monto_neto_cents: 1_000,
            moneda: "NIO".into(),
            proveedor: "tilopay".into(),
            estado: "pendiente".into(),
            referencia_externa: String::new(),
            error: String::new(),
            creado_en: "2026-11-25T00:00:00.000Z".into(),
        }
    }

    fn cuenta(referencia: &str) -> CuentaPago {
        CuentaPago {
            id: "00000000-0000-0000-0000-000000000003".into(),
            beneficiario_tipo: "vendedor".into(),
            beneficiario_id: "00000000-0000-0000-0000-000000000002".into(),
            proveedor: "tilopay".into(),
            referencia_externa: referencia.into(),
            moneda: "NIO".into(),
            activo: true,
        }
    }

    async fn aceptar() -> AxumJson<Value> {
        AxumJson(json!({ "referencia": "ext-1" }))
    }

    async fn rechazar() -> (axum::http::StatusCode, AxumJson<Value>) {
        (
            axum::http::StatusCode::BAD_GATEWAY,
            AxumJson(json!({ "message": "rechazado" })),
        )
    }

    #[tokio::test]
    async fn debe_validar_proveedor_url_y_cuenta() {
        assert_eq!(
            DispersionHttp::nuevo("otro")
                .expect_err("proveedor invalido")
                .codigo,
            "DISPERSION_PROVEEDOR_INVALIDO"
        );

        std::env::remove_var("PAGADITO_DISPERSION_URL");
        let sin_url = DispersionHttp::nuevo("pagadito").expect("adapter");
        assert_eq!(
            sin_url
                .enviar(&payout(), &cuenta("cta-1"))
                .await
                .expect_err("sin url")
                .codigo,
            "DISPERSION_NO_CONFIGURADA"
        );
    }

    #[tokio::test]
    async fn debe_enviar_y_traducir_respuestas_del_proveedor() {
        let app = Router::new()
            .route("/payout", post(aceptar))
            .route("/rechazo", post(rechazar));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("puerto mock");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let servidor = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("servidor mock");
        });

        std::env::set_var("TILOPAY_DISPERSION_URL", &base);
        std::env::set_var("TILOPAY_DISPERSION_PATH", "/payout");
        std::env::set_var("TILOPAY_DISPERSION_TOKEN", "token");
        let adapter = DispersionHttp::nuevo("tilopay").expect("adapter");
        assert_eq!(
            adapter
                .enviar(&payout(), &cuenta("cta-1"))
                .await
                .expect("envio"),
            "ext-1"
        );
        assert_eq!(
            adapter.consultar("ext-1").await.expect("consulta"),
            "pendiente"
        );
        assert_eq!(
            adapter
                .enviar(&payout(), &cuenta(""))
                .await
                .expect_err("cuenta vacia")
                .codigo,
            "CUENTA_PAGO_INCOMPLETA"
        );

        std::env::set_var("TILOPAY_DISPERSION_PATH", "/rechazo");
        let rechazo = DispersionHttp::nuevo("tilopay").expect("adapter");
        assert_eq!(
            rechazo
                .enviar(&payout(), &cuenta("cta-1"))
                .await
                .expect_err("rechazo")
                .codigo,
            "DISPERSION_RECHAZADA"
        );

        std::env::remove_var("TILOPAY_DISPERSION_URL");
        std::env::remove_var("TILOPAY_DISPERSION_PATH");
        std::env::remove_var("TILOPAY_DISPERSION_TOKEN");
        servidor.abort();
    }

    #[tokio::test]
    async fn debe_fallar_cerrado_el_proveedor_no_disponible() {
        let proveedor = DispersionNoDisponible;
        assert_eq!(proveedor.nombre(), "no-disponible");
        assert_eq!(
            proveedor
                .enviar(&payout(), &cuenta("cta"))
                .await
                .expect_err("no disponible")
                .codigo,
            "SERVICIO_NO_DISPONIBLE"
        );
    }
}
