//! Configuración de runtime del servicio.
//!
//! Réplica de `@core/shared` (`PUERTOS`, `NOMBRE_SERVICIOS`) y de las reglas
//! R-MS-5: las variables críticas se validan al arrancar y el proceso falla
//! rápido si faltan.

use std::env;

pub const NOMBRE_SERVICIO: &str = "finance-service";
pub const PUERTO_FINANCE: u16 = 3007;
pub const TIMEOUT_INTERNO_DEFAULT_MS: u64 = 3000;
pub const TASA_COMISION_DEFAULT: f64 = 0.12;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub puerto: u16,
    pub servicio: String,
    pub database_url: Option<String>,
    pub rabbitmq_url: Option<String>,
    pub internal_api_key: Option<String>,
    pub orders_service_url: String,
    pub identity_service_url: String,
    pub internal_request_timeout_ms: u64,
    pub commission_rate: f64,
    pub emitir_comprobantes_fiscales: bool,
    pub outbox_tabla: String,
}

impl Config {
    /// Carga la configuración del entorno. `DATABASE_URL` y `RABBITMQ_URL` son
    /// obligatorias en el arranque real; se exponen como `Option` para que los
    /// tests puedan construir la configuración sin infraestructura.
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_pairs(env::vars())
    }

    pub fn from_pairs<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut map = std::collections::HashMap::new();
        for (clave, valor) in vars {
            map.insert(clave.into(), valor.into());
        }
        let leer = |clave: &str| {
            map.get(clave)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        let puerto = match leer("PORT") {
            Some(valor) => valor
                .parse::<u16>()
                .map_err(|_| ConfigError::Invalida("PORT", valor))?,
            None => PUERTO_FINANCE,
        };

        let commission_rate = match leer("COMMISSION_RATE") {
            Some(valor) => valor
                .parse::<f64>()
                .map_err(|_| ConfigError::Invalida("COMMISSION_RATE", valor))?,
            None => TASA_COMISION_DEFAULT,
        };
        if !(0.0..1.0).contains(&commission_rate) {
            return Err(ConfigError::Invalida(
                "COMMISSION_RATE",
                commission_rate.to_string(),
            ));
        }

        Ok(Self {
            puerto,
            servicio: leer("SERVICIO").unwrap_or_else(|| NOMBRE_SERVICIO.to_string()),
            database_url: leer("DATABASE_URL"),
            rabbitmq_url: leer("RABBITMQ_URL"),
            internal_api_key: leer("INTERNAL_API_KEY"),
            orders_service_url: leer("ORDERS_SERVICE_URL")
                .unwrap_or_else(|| "http://orders-service:3004".to_string())
                .trim_end_matches('/')
                .to_string(),
            identity_service_url: leer("IDENTITY_SERVICE_URL")
                .unwrap_or_else(|| "http://identity-service:3001".to_string())
                .trim_end_matches('/')
                .to_string(),
            internal_request_timeout_ms: match leer("INTERNAL_REQUEST_TIMEOUT_MS") {
                Some(valor) => match valor.parse::<u64>() {
                    Ok(numero) if numero > 0 => numero,
                    _ => TIMEOUT_INTERNO_DEFAULT_MS,
                },
                None => TIMEOUT_INTERNO_DEFAULT_MS,
            },
            commission_rate,
            emitir_comprobantes_fiscales: leer("EMITIR_COMPROBANTES_FISCALES")
                .map(|valor| valor.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            outbox_tabla: leer("OUTBOX_TABLA").unwrap_or_else(|| "outbox".to_string()),
        })
    }

    /// Verificación de arranque del servicio completo (equivalente a la de
    /// `PgService`/`RabbitService`: sin estas variables no se levanta).
    pub fn validar_arranque(&self) -> Result<(), ConfigError> {
        if self.database_url.is_none() {
            return Err(ConfigError::Faltante("DATABASE_URL"));
        }
        if self.rabbitmq_url.is_none() {
            return Err(ConfigError::Faltante("RABBITMQ_URL"));
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ConfigError {
    #[error("{0} es obligatoria para arrancar el servicio.")]
    Faltante(&'static str),
    #[error("{0} tiene un valor invalido: {1}")]
    Invalida(&'static str, String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_usar_defaults_cuando_faltan_variables_opcionales() {
        let config = Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config");
        assert_eq!(config.puerto, PUERTO_FINANCE);
        assert_eq!(config.servicio, NOMBRE_SERVICIO);
        assert_eq!(config.orders_service_url, "http://orders-service:3004");
        assert_eq!(config.identity_service_url, "http://identity-service:3001");
        assert_eq!(
            config.internal_request_timeout_ms,
            TIMEOUT_INTERNO_DEFAULT_MS
        );
        assert_eq!(config.commission_rate, TASA_COMISION_DEFAULT);
        assert!(!config.emitir_comprobantes_fiscales);
        assert_eq!(config.outbox_tabla, "outbox");
        assert_eq!(config.database_url, None);
    }

    #[test]
    fn debe_leer_y_limpiar_las_variables_de_entorno() {
        let config = Config::from_pairs([
            ("PORT", "4007"),
            ("SERVICIO", " finance-service "),
            ("DATABASE_URL", "postgres://localhost/core"),
            ("RABBITMQ_URL", "amqp://localhost"),
            ("ORDERS_SERVICE_URL", "http://orders:3004/"),
            ("IDENTITY_SERVICE_URL", "http://identity:3001/"),
            ("INTERNAL_REQUEST_TIMEOUT_MS", "1500"),
            ("COMMISSION_RATE", "0.1"),
            ("EMITIR_COMPROBANTES_FISCALES", "true"),
            ("OUTBOX_TABLA", "finance.outbox"),
        ])
        .expect("config");
        assert_eq!(config.puerto, 4007);
        assert_eq!(config.servicio, "finance-service");
        assert_eq!(
            config.database_url.as_deref(),
            Some("postgres://localhost/core")
        );
        assert_eq!(config.orders_service_url, "http://orders:3004");
        assert_eq!(config.identity_service_url, "http://identity:3001");
        assert_eq!(config.internal_request_timeout_ms, 1500);
        assert_eq!(config.commission_rate, 0.1);
        assert!(config.emitir_comprobantes_fiscales);
        assert_eq!(config.outbox_tabla, "finance.outbox");
    }

    #[test]
    fn debe_rechazar_un_puerto_invalido() {
        let error = Config::from_pairs([("PORT", "abc")]).expect_err("puerto invalido");
        assert_eq!(error, ConfigError::Invalida("PORT", "abc".to_string()));
    }

    #[test]
    fn debe_rechazar_una_tasa_de_comision_fuera_de_rango() {
        let error =
            Config::from_pairs([("COMMISSION_RATE", "1.5")]).expect_err("comision invalida");
        assert!(matches!(error, ConfigError::Invalida("COMMISSION_RATE", _)));
    }

    #[test]
    fn debe_descartar_timeouts_invalidos() {
        for valor in ["0", "no-numerico"] {
            let config =
                Config::from_pairs([("INTERNAL_REQUEST_TIMEOUT_MS", valor)]).expect("config");
            assert_eq!(
                config.internal_request_timeout_ms,
                TIMEOUT_INTERNO_DEFAULT_MS
            );
        }
    }

    #[test]
    fn debe_fallar_el_arranque_sin_database_url() {
        let config = Config::from_pairs([("RABBITMQ_URL", "amqp://localhost")]).expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Faltante("DATABASE_URL"))
        );
    }

    #[test]
    fn debe_fallar_el_arranque_sin_rabbitmq_url() {
        let config =
            Config::from_pairs([("DATABASE_URL", "postgres://localhost/core")]).expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Faltante("RABBITMQ_URL"))
        );
    }
}
