//! Configuración de runtime del servicio.
//!
//! Réplica de `@core/shared` (`PUERTOS`, `NOMBRE_SERVICIOS`) y de las reglas
//! R-MS-5: las variables críticas se validan al arrancar y el proceso falla
//! rápido si faltan.

use std::env;

pub const NOMBRE_SERVICIO: &str = "catalog-service";
pub const PUERTO_CATALOG: u16 = 3002;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub puerto: u16,
    pub servicio: String,
    pub database_url: Option<String>,
    pub rabbitmq_url: Option<String>,
    pub internal_api_key: Option<String>,
    pub identity_service_url: String,
    pub storage_service_url: String,
    pub outbox_tabla: String,
    pub internal_request_timeout_ms: u64,
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
        let entero = |clave: &str, por_defecto: u64| match leer(clave) {
            Some(valor) => match valor.parse::<u64>() {
                Ok(numero) if numero > 0 => numero,
                _ => por_defecto,
            },
            None => por_defecto,
        };

        let puerto = match leer("PORT") {
            Some(valor) => valor
                .parse::<u16>()
                .map_err(|_| ConfigError::Invalida("PORT", valor))?,
            None => PUERTO_CATALOG,
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
            storage_service_url: leer("STORAGE_SERVICE_URL")
                .unwrap_or_else(|| "http://storage-service:3010".to_string())
                .trim_end_matches('/')
                .to_string(),
            outbox_tabla: leer("OUTBOX_TABLA").unwrap_or_else(|| "catalog.outbox".to_string()),
            internal_request_timeout_ms: entero("INTERNAL_REQUEST_TIMEOUT_MS", 3_000),
        })
    }

    /// Verificación de arranque: sin dependencias críticas el servicio no se
    /// levanta, igual que `PgService` y `RabbitService`.
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
        assert_eq!(config.puerto, PUERTO_CATALOG);
        assert_eq!(config.servicio, NOMBRE_SERVICIO);
        assert_eq!(config.identity_service_url, "http://identity-service:3001");
        assert_eq!(config.storage_service_url, "http://storage-service:3010");
        assert_eq!(config.outbox_tabla, "catalog.outbox");
        assert_eq!(config.internal_request_timeout_ms, 3000);
        assert_eq!(config.database_url, None);
        assert_eq!(config.rabbitmq_url, None);
    }

    #[test]
    fn debe_leer_y_limpiar_las_variables_de_entorno() {
        let config = Config::from_pairs([
            ("PORT", "4002"),
            ("SERVICIO", " catalog-service "),
            ("DATABASE_URL", "postgres://localhost/core"),
            ("RABBITMQ_URL", "amqp://localhost"),
            ("IDENTITY_SERVICE_URL", "http://identity:3001/"),
            ("STORAGE_SERVICE_URL", "http://storage:3010/"),
            ("OUTBOX_TABLA", "catalog.outbox"),
            ("INTERNAL_REQUEST_TIMEOUT_MS", "2500"),
        ])
        .expect("config");
        assert_eq!(config.puerto, 4002);
        assert_eq!(config.servicio, "catalog-service");
        assert_eq!(
            config.database_url.as_deref(),
            Some("postgres://localhost/core")
        );
        assert_eq!(config.rabbitmq_url.as_deref(), Some("amqp://localhost"));
        assert_eq!(config.identity_service_url, "http://identity:3001");
        assert_eq!(config.storage_service_url, "http://storage:3010");
        assert_eq!(config.outbox_tabla, "catalog.outbox");
        assert_eq!(config.internal_request_timeout_ms, 2500);
    }

    #[test]
    fn debe_rechazar_un_puerto_invalido() {
        let error = Config::from_pairs([("PORT", "abc")]).expect_err("puerto invalido");
        assert_eq!(error, ConfigError::Invalida("PORT", "abc".to_string()));
    }

    #[test]
    fn debe_fallar_el_arranque_sin_dependencias_criticas() {
        let config = Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config");
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

    #[test]
    fn debe_aceptar_el_arranque_con_las_dos_urls() {
        let config = Config::from_pairs([
            ("DATABASE_URL", "postgres://localhost/core"),
            ("RABBITMQ_URL", "amqp://localhost"),
        ])
        .expect("config");
        assert!(config.validar_arranque().is_ok());
    }
}
