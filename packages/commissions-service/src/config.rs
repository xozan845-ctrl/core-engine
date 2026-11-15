//! Configuración de runtime del servicio.
//!
//! Réplica de `@core/shared` (`PUERTOS`, `NOMBRE_SERVICIOS`) y de las reglas
//! R-MS-5: las variables críticas se validan al arrancar y el proceso falla
//! rápido si faltan.

use std::env;

pub const NOMBRE_SERVICIO: &str = "commissions-service";
pub const PUERTO_COMMISSIONS: u16 = 3006;
pub const TASA_COMISION_DEFAULT: f64 = 0.12;
pub const TIMEOUT_INTERNO_DEFAULT_MS: u64 = 3000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub puerto: u16,
    pub servicio: String,
    pub database_url: Option<String>,
    pub rabbitmq_url: Option<String>,
    pub internal_api_key: Option<String>,
    pub orders_service_url: String,
    pub catalog_service_url: String,
    pub identity_service_url: String,
    pub internal_request_timeout_ms: u64,
    pub outbox_tabla: String,
    /// Valor crudo de `COMMISSION_RATE`; se interpreta con la misma semántica
    /// que `Number(...)` de JavaScript (un valor inválido produce `TASA_INVALIDA`
    /// al calcular, no en el arranque).
    pub commission_rate: Option<String>,
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
            None => PUERTO_COMMISSIONS,
        };

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
            catalog_service_url: leer("CATALOG_SERVICE_URL")
                .unwrap_or_else(|| "http://catalog-service:3002".to_string())
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
            outbox_tabla: leer("OUTBOX_TABLA").unwrap_or_else(|| "outbox".to_string()),
            commission_rate: map
                .get("COMMISSION_RATE")
                .map(|valor| valor.trim().to_string()),
        })
    }

    /// Tasa efectiva con la semántica de `Number(process.env.COMMISSION_RATE ?? 0.12)`.
    pub fn tasa_comision(&self) -> f64 {
        match &self.commission_rate {
            None => TASA_COMISION_DEFAULT,
            Some(valor) => valor.parse::<f64>().unwrap_or(f64::NAN),
        }
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
        assert_eq!(config.puerto, PUERTO_COMMISSIONS);
        assert_eq!(config.servicio, NOMBRE_SERVICIO);
        assert_eq!(config.orders_service_url, "http://orders-service:3004");
        assert_eq!(config.catalog_service_url, "http://catalog-service:3002");
        assert_eq!(config.identity_service_url, "http://identity-service:3001");
        assert_eq!(
            config.internal_request_timeout_ms,
            TIMEOUT_INTERNO_DEFAULT_MS
        );
        assert_eq!(config.outbox_tabla, "outbox");
        assert_eq!(config.database_url, None);
        assert_eq!(config.tasa_comision(), TASA_COMISION_DEFAULT);
    }

    #[test]
    fn debe_leer_y_limpiar_las_variables_de_entorno() {
        let config = Config::from_pairs([
            ("PORT", "4006"),
            ("SERVICIO", " commissions-service "),
            ("DATABASE_URL", "postgres://localhost/core"),
            ("RABBITMQ_URL", "amqp://localhost"),
            ("ORDERS_SERVICE_URL", "http://orders:3004/"),
            ("CATALOG_SERVICE_URL", "http://catalog:3002/"),
            ("IDENTITY_SERVICE_URL", "http://identity:3001/"),
            ("OUTBOX_TABLA", "commissions.outbox"),
            ("COMMISSION_RATE", "0.15"),
        ])
        .expect("config");
        assert_eq!(config.puerto, 4006);
        assert_eq!(config.servicio, "commissions-service");
        assert_eq!(
            config.database_url.as_deref(),
            Some("postgres://localhost/core")
        );
        assert_eq!(config.orders_service_url, "http://orders:3004");
        assert_eq!(config.catalog_service_url, "http://catalog:3002");
        assert_eq!(config.identity_service_url, "http://identity:3001");
        assert_eq!(config.outbox_tabla, "commissions.outbox");
        assert!((config.tasa_comision() - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn debe_rechazar_un_puerto_invalido() {
        let error = Config::from_pairs([("PORT", "abc")]).expect_err("puerto invalido");
        assert_eq!(error, ConfigError::Invalida("PORT", "abc".to_string()));
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

    #[test]
    fn debe_interpretar_una_tasa_invalida_como_nan() {
        let config = Config::from_pairs([("COMMISSION_RATE", "no-es-numero")]).expect("config");
        assert!(config.tasa_comision().is_nan());
        let vacia = Config::from_pairs([("COMMISSION_RATE", "")]).expect("config");
        assert!(vacia.tasa_comision().is_nan());
    }
}
