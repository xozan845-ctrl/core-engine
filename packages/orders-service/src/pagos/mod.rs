//! Métodos de pago configurables y registro de pagos por orden.
//!
//! El catálogo `orders.metodos_pago` es global y lo administra un admin. El
//! registro `orders.pagos` guarda intentos/confirmaciones; la confirmación
//! manual (efectivo/transferencia) mueve la orden a `pagada`.

mod comunes;
mod configuracion;
mod consulta;
mod operaciones;
mod proveedores;
mod webhooks;

mod prelude {
    pub(crate) use crate::db::{iso_ms, uuid_opcional, uuid_texto, Db};
    pub(crate) use crate::errors::ApiError;
    pub(crate) use crate::pasarelas::{pagadito::Pagadito, tilopay::Tilopay};
    pub(crate) use chrono::{DateTime, Utc};
    pub(crate) use reqwest::Client;
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::Value;
    pub(crate) use sqlx::Row;
    pub(crate) use std::sync::Arc;
    pub(crate) use std::time::Duration;
    pub(crate) use uuid::Uuid;
}
use self::prelude::*;
use serde_json::json;

pub use proveedores::{
    CargoExterno, EstadoExterno, ProveedorHttp, ProveedorManual, ProveedorPagos, ProveedorSimulado,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetodoPago {
    pub codigo: String,
    pub nombre: String,
    pub requiere_confirmacion: bool,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProveedorPago {
    pub codigo: String,
    pub nombre: String,
    pub tipo: String,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfiguracionPagos {
    pub empresa_id: String,
    pub negocio_id: Option<String>,
    pub proveedor_codigo: String,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pago {
    pub id: String,
    pub order_id: String,
    pub empresa_id: Option<String>,
    pub metodo_codigo: String,
    pub estado: String,
    pub monto_cents: i64,
    pub moneda: String,
    pub referencia: String,
    /// Identificador de la transacción en la pasarela (orderNumber de
    /// Tilopay, token de Pagadito). Vacío hasta que la pasarela lo informa.
    /// No se expone en la API: es un detalle operativo de la pasarela.
    #[serde(default, skip_serializing)]
    pub referencia_proveedor: String,
    pub proveedor: String,
    pub monto_reembolsado_cents: i64,
    pub motivo_rechazo: String,
    pub confirmado_por: Option<String>,
    pub conciliado_en: Option<String>,
    pub confirmado_en: Option<String>,
    pub creado_en: String,
}

pub struct Pagos {
    db: Arc<Db>,
    proveedor_default: Arc<dyn ProveedorPagos>,
}

impl Pagos {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self {
            db,
            proveedor_default: Arc::new(ProveedorSimulado),
        }
    }

    /// Proveedor global de fallback. Falla si la configuración declarada no
    /// puede construirse: arrancar simulando pagos cuando se pidió una pasarela
    /// real sería peor que no arrancar.
    pub fn con_config(db: Arc<Db>, config: &crate::config::Config) -> Result<Self, ApiError> {
        let tipo = config.pago_proveedor.trim().to_ascii_lowercase();
        let proveedor: Arc<dyn ProveedorPagos> = match tipo.as_str() {
            "http" => {
                let url = config.pago_proveedor_url.clone().ok_or_else(|| {
                    ApiError::dominio(
                        "PROVEEDOR_PAGO_INCOMPLETO",
                        "PAGO_PROVEEDOR=http requiere PAGO_PROVEEDOR_URL.",
                    )
                })?;
                Arc::new(ProveedorHttp::nuevo(url, config.pago_proveedor_key.clone()))
            }
            "tilopay" => {
                let mut configuracion = json!({});
                if let Some(url) = config.pago_proveedor_url.clone() {
                    configuracion["base_url"] = json!(url);
                }
                Arc::new(Tilopay::nuevo(&configuracion)?)
            }
            "pagadito" => {
                let mut configuracion = json!({});
                if let Some(url) = config.pago_proveedor_url.clone() {
                    configuracion["base_url"] = json!(url);
                }
                Arc::new(Pagadito::nuevo(&configuracion)?)
            }
            "manual" => Arc::new(ProveedorManual),
            "simulado" => Arc::new(ProveedorSimulado),
            otro => {
                return Err(ApiError::dominio(
                    "PROVEEDOR_PAGO_DESCONOCIDO",
                    format!("PAGO_PROVEEDOR={otro} no es un tipo de proveedor válido."),
                ))
            }
        };
        Ok(Self {
            db,
            proveedor_default: proveedor,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::configuracion::{fusionar_config_tenant, validar_config_tenant};
    use super::consulta::{config_de, validar_codigo, validar_nombre};
    use super::webhooks::referencia_proveedor_de;
    use super::*;

    #[test]
    fn debe_validar_codigos_y_nombres() {
        assert_eq!(
            validar_codigo("Transferencia").expect("codigo"),
            "transferencia"
        );
        assert!(validar_codigo("1malo").is_err());
        assert!(validar_nombre("Efectivo").is_ok());
        assert!(validar_nombre("x").is_err());
    }

    #[test]
    fn debe_exigir_config_objeto() {
        assert_eq!(config_de(&json!({})).expect("config"), json!({}));
        assert!(config_de(&json!({"config": []})).is_err());
    }

    #[test]
    fn debe_rechazar_url_y_claves_de_entorno_del_tenant() {
        for clave in [
            "url",
            "clave_env",
            "webhook_secreto_env",
            "callback_url",
            "return_url",
        ] {
            let config = json!({ clave: "https://attacker.tld" });
            assert!(
                validar_config_tenant(&config).is_err(),
                "{clave} debe estar reservada"
            );
        }
        assert!(validar_config_tenant(&json!({"timeout_ms": 5000})).is_ok());
    }

    #[test]
    fn la_config_del_tenant_no_puede_sobrescribir_la_del_proveedor() {
        let base = json!({
            "url": "https://pasarela.example",
            "clave_env": "PAGO_PROVEEDOR_KEY",
            "timeout_ms": 5000
        });
        let tenant = json!({
            "url": "https://attacker.tld",
            "clave_env": "JWT_SECRET",
            "timeout_ms": 9000
        });
        let fusionada = fusionar_config_tenant(&base, &tenant);
        assert_eq!(fusionada["url"], "https://pasarela.example");
        assert_eq!(fusionada["clave_env"], "PAGO_PROVEEDOR_KEY");
        assert_eq!(fusionada["timeout_ms"], 9000);
    }

    #[test]
    fn debe_extraer_la_referencia_del_proveedor() {
        assert_eq!(
            referencia_proveedor_de("tilopay", Some(&json!({"orderNumber": " T-1 "}))),
            Some("T-1".to_string())
        );
        assert_eq!(
            referencia_proveedor_de("pagadito", Some(&json!({"resource": {"token": "tok"}}))),
            Some("tok".to_string())
        );
        assert_eq!(
            referencia_proveedor_de("pagadito", Some(&json!({"resource": {"ern": "o-1"}}))),
            None
        );
        assert_eq!(referencia_proveedor_de("tilopay", None), None);
    }

    #[tokio::test]
    async fn debe_crear_y_revertir_cargos_con_el_proveedor_simulado() {
        let proveedor = ProveedorSimulado;
        assert_eq!(proveedor.nombre(), "simulado");
        let cargo = proveedor
            .crear_cargo("orden-1", 10_000, "NIO")
            .await
            .expect("cargo");
        assert!(cargo.referencia.starts_with("sim-"));
        proveedor
            .reembolsar(&cargo.referencia, 5_000)
            .await
            .expect("reembolso");
        proveedor
            .contracargar(&cargo.referencia, 10_000, "fraude")
            .await
            .expect("contracargo");
    }
}
