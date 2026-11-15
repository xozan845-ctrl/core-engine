//! Configuración de runtime del servicio.
//!
//! Réplica de `@core/shared` (`PUERTOS`, `NOMBRE_SERVICIOS`) y de R-MS-5: las
//! variables críticas se validan al arrancar y el proceso falla rápido.

use std::env;

pub const NOMBRE_SERVICIO: &str = "market-intelligence-service";
pub const PUERTO_INTELLIGENCE: u16 = 3009;
pub const TIMEOUT_INTERNO_DEFAULT_MS: u64 = 3000;

/// Bucket de agregación para ventas que no identifican empresa (ADR-27).
pub const EMPRESA_MERCADO_ID: &str = "00000000-0000-0000-0000-000000000001";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub puerto: u16,
    pub servicio: String,
    pub database_url: Option<String>,
    pub rabbitmq_url: Option<String>,
    pub internal_api_key: Option<String>,
    pub identity_service_url: String,
    pub internal_request_timeout_ms: u64,
    pub outbox_tabla: String,
}

impl Config {
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
            None => PUERTO_INTELLIGENCE,
        };

        Ok(Self {
            puerto,
            servicio: leer("SERVICIO").unwrap_or_else(|| NOMBRE_SERVICIO.to_string()),
            database_url: leer("DATABASE_URL"),
            rabbitmq_url: leer("RABBITMQ_URL"),
            internal_api_key: leer("INTERNAL_API_KEY"),
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
            outbox_tabla: leer("OUTBOX_TABLA").unwrap_or_else(|| "outbox".to_string()),
        })
    }

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

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
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
        assert_eq!(config.puerto, PUERTO_INTELLIGENCE);
        assert_eq!(config.servicio, NOMBRE_SERVICIO);
        assert_eq!(config.identity_service_url, "http://identity-service:3001");
        assert_eq!(
            config.internal_request_timeout_ms,
            TIMEOUT_INTERNO_DEFAULT_MS
        );
        assert_eq!(config.outbox_tabla, "outbox");
        assert_eq!(config.database_url, None);
    }

    #[test]
    fn debe_leer_y_limpiar_las_variables_de_entorno() {
        let config = Config::from_pairs([
            ("PORT", "4009"),
            ("SERVICIO", " market-intelligence-service "),
            ("DATABASE_URL", "postgres://localhost/core"),
            ("RABBITMQ_URL", "amqp://localhost"),
            ("IDENTITY_SERVICE_URL", "http://identity:3001/"),
            ("OUTBOX_TABLA", "intelligence.outbox"),
        ])
        .expect("config");
        assert_eq!(config.puerto, 4009);
        assert_eq!(config.servicio, "market-intelligence-service");
        assert_eq!(config.identity_service_url, "http://identity:3001");
        assert_eq!(config.outbox_tabla, "intelligence.outbox");
    }

    #[test]
    fn debe_rechazar_un_puerto_invalido() {
        let error = Config::from_pairs([("PORT", "abc")]).expect_err("puerto invalido");
        assert_eq!(error, ConfigError::Invalida("PORT", "abc".to_string()));
    }

    #[test]
    fn debe_fallar_el_arranque_sin_variables_obligatorias() {
        let config = Config::from_pairs([("RABBITMQ_URL", "amqp://localhost")]).expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Faltante("DATABASE_URL"))
        );
        let config =
            Config::from_pairs([("DATABASE_URL", "postgres://localhost/core")]).expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Faltante("RABBITMQ_URL"))
        );
    }
}
