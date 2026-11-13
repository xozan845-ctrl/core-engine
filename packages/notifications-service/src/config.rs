//! Configuración de runtime del servicio.
//!
//! Réplica de `@core/shared` (`PUERTOS`, `NOMBRE_SERVICIOS`) y de las reglas
//! R-MS-5: las variables críticas se validan al arrancar y el proceso falla
//! rápido si faltan. Incluye la validación de `EMAIL_MODE` (ADR-0003), que en
//! NestJS ejecutaba la fábrica de `EmailModule`.

use std::env;

pub const NOMBRE_SERVICIO: &str = "notifications-service";
pub const PUERTO_NOTIFICATIONS: u16 = 3012;
pub const SMTP_PUERTO_DEFAULT: u16 = 587;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub puerto: u16,
    pub servicio: String,
    pub database_url: Option<String>,
    pub rabbitmq_url: Option<String>,
    pub internal_api_key: Option<String>,
    pub orders_service_url: String,
    pub identity_service_url: String,
    pub internal_request_timeout_ms: u64,
    pub email: EmailConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailMode {
    Log,
    Smtp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailConfig {
    pub modo: EmailMode,
    pub from: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_password: Option<String>,
}

impl EmailConfig {
    /// Validación equivalente a `crearEmailProvider`: SMTP exige host, puerto
    /// válido y remitente; usuario y contraseña van juntos o ninguno.
    pub fn validar(&self) -> Result<(), ConfigError> {
        if self.modo == EmailMode::Log {
            return Ok(());
        }
        if self.smtp_host.is_none() || self.from.is_none() {
            return Err(ConfigError::Email(
                "Con EMAIL_MODE=smtp son obligatorios SMTP_HOST, SMTP_PORT y EMAIL_FROM."
                    .to_string(),
            ));
        }
        if self.smtp_user.is_some() != self.smtp_password.is_some() {
            return Err(ConfigError::Email(
                "SMTP_USER y SMTP_PASSWORD deben configurarse juntos o ninguno.".to_string(),
            ));
        }
        Ok(())
    }

    pub fn es_seguro(&self) -> bool {
        self.smtp_port == 465
    }
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
            None => PUERTO_NOTIFICATIONS,
        };

        let modo = match leer("EMAIL_MODE")
            .unwrap_or_else(|| "log".to_string())
            .to_lowercase()
            .as_str()
        {
            "log" => EmailMode::Log,
            "smtp" => EmailMode::Smtp,
            otro => return Err(ConfigError::Invalida("EMAIL_MODE", otro.to_string())),
        };

        let smtp_port =
            match map.get("SMTP_PORT").map(|valor| valor.trim()) {
                Some(valor) => match valor.parse::<u16>() {
                    Ok(numero) if numero > 0 => numero,
                    _ if modo == EmailMode::Smtp => return Err(ConfigError::Email(
                        "Con EMAIL_MODE=smtp son obligatorios SMTP_HOST, SMTP_PORT y EMAIL_FROM."
                            .to_string(),
                    )),
                    _ => SMTP_PUERTO_DEFAULT,
                },
                None => SMTP_PUERTO_DEFAULT,
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
            identity_service_url: leer("IDENTITY_SERVICE_URL")
                .unwrap_or_else(|| "http://identity-service:3001".to_string())
                .trim_end_matches('/')
                .to_string(),
            internal_request_timeout_ms: match leer("INTERNAL_REQUEST_TIMEOUT_MS") {
                Some(valor) => match valor.parse::<u64>() {
                    Ok(numero) if numero > 0 => numero,
                    _ => 3000,
                },
                None => 3000,
            },
            email: EmailConfig {
                modo,
                from: leer("EMAIL_FROM"),
                smtp_host: leer("SMTP_HOST"),
                smtp_port,
                smtp_user: leer("SMTP_USER"),
                smtp_password: leer("SMTP_PASSWORD"),
            },
        })
    }

    /// Verificación de arranque del servicio completo (equivalente a la de
    /// `PgService`/`RabbitService` y `email.provider`: sin estas variables no
    /// se levanta).
    pub fn validar_arranque(&self) -> Result<(), ConfigError> {
        if self.database_url.is_none() {
            return Err(ConfigError::Faltante("DATABASE_URL"));
        }
        if self.rabbitmq_url.is_none() {
            return Err(ConfigError::Faltante("RABBITMQ_URL"));
        }
        self.email.validar()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{0} es obligatoria para arrancar el servicio.")]
    Faltante(&'static str),
    #[error("{0} tiene un valor invalido: {1}")]
    Invalida(&'static str, String),
    #[error("{0}")]
    Email(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_usar_defaults_cuando_faltan_variables_opcionales() {
        let config = Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config");
        assert_eq!(config.puerto, PUERTO_NOTIFICATIONS);
        assert_eq!(config.servicio, NOMBRE_SERVICIO);
        assert_eq!(config.orders_service_url, "http://orders-service:3004");
        assert_eq!(config.identity_service_url, "http://identity-service:3001");
        assert_eq!(config.internal_request_timeout_ms, 3000);
        assert_eq!(config.database_url, None);
        assert_eq!(config.email.modo, EmailMode::Log);
        assert_eq!(config.email.smtp_port, SMTP_PUERTO_DEFAULT);
    }

    #[test]
    fn debe_leer_y_limpiar_las_variables_de_entorno() {
        let config = Config::from_pairs([
            ("PORT", "4012"),
            ("SERVICIO", " notifications-service "),
            ("DATABASE_URL", "postgres://localhost/core"),
            ("RABBITMQ_URL", "amqp://localhost"),
            ("ORDERS_SERVICE_URL", "http://orders:3004/"),
            ("IDENTITY_SERVICE_URL", "http://identity:3001/"),
            ("INTERNAL_REQUEST_TIMEOUT_MS", "1500"),
        ])
        .expect("config");
        assert_eq!(config.puerto, 4012);
        assert_eq!(config.servicio, "notifications-service");
        assert_eq!(
            config.database_url.as_deref(),
            Some("postgres://localhost/core")
        );
        assert_eq!(config.orders_service_url, "http://orders:3004");
        assert_eq!(config.identity_service_url, "http://identity:3001");
        assert_eq!(config.internal_request_timeout_ms, 1500);
    }

    #[test]
    fn debe_rechazar_un_puerto_invalido() {
        let error = Config::from_pairs([("PORT", "abc")]).expect_err("puerto invalido");
        assert_eq!(error, ConfigError::Invalida("PORT", "abc".to_string()));
    }

    #[test]
    fn debe_descartar_timeouts_invalidos() {
        for valor in ["0", "no-numerico"] {
            let config =
                Config::from_pairs([("INTERNAL_REQUEST_TIMEOUT_MS", valor)]).expect("config");
            assert_eq!(config.internal_request_timeout_ms, 3000);
        }
    }

    #[test]
    fn debe_aceptar_log_y_rechazar_modos_de_correo_invalidos() {
        assert_eq!(
            Config::from_pairs([("EMAIL_MODE", " LOG ")])
                .expect("log")
                .email
                .modo,
            EmailMode::Log
        );
        assert_eq!(
            Config::from_pairs([("EMAIL_MODE", "smtp")])
                .expect("smtp")
                .email
                .modo,
            EmailMode::Smtp
        );
        assert_eq!(
            Config::from_pairs([("EMAIL_MODE", "otro")]).expect_err("invalido"),
            ConfigError::Invalida("EMAIL_MODE", "otro".to_string())
        );
    }

    #[test]
    fn debe_exigir_configuracion_smtp_completa() {
        let config = Config::from_pairs([("EMAIL_MODE", "smtp")]).expect("config");
        assert!(matches!(
            config.validar_arranque(),
            Err(ConfigError::Faltante("DATABASE_URL"))
        ));

        let config = Config::from_pairs([
            ("DATABASE_URL", "postgres://localhost/core"),
            ("RABBITMQ_URL", "amqp://localhost"),
            ("EMAIL_MODE", "smtp"),
        ])
        .expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Email(
                "Con EMAIL_MODE=smtp son obligatorios SMTP_HOST, SMTP_PORT y EMAIL_FROM."
                    .to_string()
            ))
        );

        let config = Config::from_pairs([
            ("DATABASE_URL", "postgres://localhost/core"),
            ("RABBITMQ_URL", "amqp://localhost"),
            ("EMAIL_MODE", "smtp"),
            ("SMTP_HOST", "smtp.test"),
            ("SMTP_PORT", "465"),
            ("EMAIL_FROM", "no-reply@test"),
            ("SMTP_USER", "usuario"),
        ])
        .expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Email(
                "SMTP_USER y SMTP_PASSWORD deben configurarse juntos o ninguno.".to_string()
            ))
        );
        assert!(config.email.es_seguro());
    }

    #[test]
    fn debe_rechazar_puerto_smtp_invalido_en_modo_smtp() {
        for puerto in ["", "abc", "0"] {
            let error = Config::from_pairs([("EMAIL_MODE", "smtp"), ("SMTP_PORT", puerto)])
                .expect_err("puerto invalido");
            assert!(matches!(error, ConfigError::Email(_)));
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
