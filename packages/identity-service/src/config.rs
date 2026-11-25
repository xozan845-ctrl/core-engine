//! Configuración de runtime de `identity-service`.
//!
//! Mismos defaults y fail-fast que `@core/shared` (`secretoJwt`,
//! `passwordAdminInicial`, `emailAdminInicial`) y las reglas R-MS-5: en
//! producción un secreto ausente detiene el arranque, nunca se degrada a un
//! valor conocido.

use crate::purga::ServicioPurga;
use base64::Engine;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::env;

pub const NOMBRE_SERVICIO: &str = "identity-service";
pub const PUERTO_IDENTITY: u16 = 3001;
pub const JWT_ACCESS_TTL_DEFAULT: &str = "900s";
pub const JWT_REFRESH_TTL_DEFAULT: &str = "7d";
pub const ADMIN_EMAIL_DEFAULT: &str = "admin@core-engine.test";
pub const ADMIN_PASSWORD_DEFAULT: &str = "Fuego-Laguna-2026!";
pub const JWT_SECRET_DEFAULT: &str = "dev_secret";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub puerto: u16,
    pub servicio: String,
    pub produccion: bool,
    pub database_url: Option<String>,
    pub rabbitmq_url: Option<String>,
    pub outbox_tabla: String,
    pub jwt_secret: String,
    pub jwt_access_ttl: String,
    pub jwt_refresh_ttl: String,
    pub frontend_url: String,
    pub storage_service_url: String,
    pub internal_api_key: Option<String>,
    pub stores_service_url: String,
    pub platform_tenant_id: Option<String>,
    pub platform_logistics_tenant_id: Option<String>,
    pub admin_email: String,
    pub admin_password: String,
    pub email_mode: String,
    pub email_from: Option<String>,
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    /// Permite SMTP sin TLS solo en desarrollo y sin credenciales (Mailpit).
    pub smtp_tls_enabled: bool,
    pub smtp_user: Option<String>,
    pub smtp_password: Option<String>,
    /// Exige correo verificado para login/refresh. Por defecto se activa en
    /// producción y queda desactivado en desarrollo/tests.
    pub exigir_correo_verificado: bool,
    /// Clave base64 de 32 bytes para cifrar secretos MFA en reposo. En
    /// producción es obligatoria; en desarrollo se deriva del JWT.
    pub mfa_encryption_key: Option<String>,
    /// Servicios a los que se propaga la purga de un tenant.
    pub servicios_purga: Vec<ServicioPurga>,
    /// Días que se conserva `identity.auditoria`; `0` desactiva la limpieza.
    pub auditoria_retencion_dias: i64,
    /// `anonimizar` conserva el evento y redacta PII; `eliminar` borra filas.
    pub auditoria_retencion_modo: String,
    /// Clave HMAC para firmar exportaciones; por defecto usa `JWT_SECRET`.
    pub export_signing_key: Option<String>,
    /// Plan comercial que habilita operar más de un negocio (ADR-0046).
    pub plataforma_plan_codigo: String,
    /// Empresas/negocios incluidos sin cobro antes de exigir el plan.
    pub plataforma_empresas_gratis: i64,
    pub plataforma_negocios_gratis: i64,
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
        let mut map = HashMap::new();
        for (clave, valor) in vars {
            map.insert(clave.into(), valor.into());
        }
        let leer = |clave: &str| {
            map.get(clave)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let produccion = leer("NODE_ENV")
            .map(|valor| valor.eq_ignore_ascii_case("production"))
            .unwrap_or(false);

        let requerido_en_produccion = |clave: &'static str, default: &str| match leer(clave) {
            Some(valor) => Ok(valor),
            None if produccion => Err(ConfigError::Faltante(clave)),
            None => Ok(default.to_string()),
        };

        let puerto = match leer("PORT") {
            Some(valor) => valor
                .parse::<u16>()
                .map_err(|_| ConfigError::Invalida("PORT", valor))?,
            None => PUERTO_IDENTITY,
        };

        Ok(Self {
            puerto,
            servicio: leer("SERVICIO").unwrap_or_else(|| NOMBRE_SERVICIO.to_string()),
            produccion,
            database_url: leer("DATABASE_URL"),
            rabbitmq_url: leer("RABBITMQ_URL"),
            outbox_tabla: leer("OUTBOX_TABLA").unwrap_or_else(|| "identity.outbox".to_string()),
            jwt_secret: requerido_en_produccion("JWT_SECRET", JWT_SECRET_DEFAULT)?,
            jwt_access_ttl: leer("JWT_ACCESS_TTL")
                .unwrap_or_else(|| JWT_ACCESS_TTL_DEFAULT.to_string()),
            jwt_refresh_ttl: leer("JWT_REFRESH_TTL")
                .unwrap_or_else(|| JWT_REFRESH_TTL_DEFAULT.to_string()),
            frontend_url: leer("FRONTEND_URL")
                .unwrap_or_else(|| "http://localhost:5173".to_string())
                .trim_end_matches('/')
                .to_string(),
            storage_service_url: leer("STORAGE_SERVICE_URL")
                .unwrap_or_else(|| "http://storage-service:3010".to_string())
                .trim_end_matches('/')
                .to_string(),
            internal_api_key: leer("INTERNAL_API_KEY"),
            stores_service_url: leer("STORES_SERVICE_URL")
                .unwrap_or_else(|| "http://stores-service:3003".to_string())
                .trim_end_matches('/')
                .to_string(),
            platform_tenant_id: leer("PLATFORM_TENANT_ID"),
            platform_logistics_tenant_id: leer("PLATFORM_LOGISTICS_TENANT_ID"),
            admin_email: requerido_en_produccion("ADMIN_EMAIL", ADMIN_EMAIL_DEFAULT)?,
            admin_password: requerido_en_produccion("ADMIN_PASSWORD", ADMIN_PASSWORD_DEFAULT)?,
            email_mode: leer("EMAIL_MODE")
                .unwrap_or_else(|| "log".to_string())
                .to_lowercase(),
            email_from: leer("EMAIL_FROM"),
            smtp_host: leer("SMTP_HOST"),
            smtp_port: match leer("SMTP_PORT") {
                Some(valor) => valor
                    .parse::<u16>()
                    .ok()
                    .filter(|puerto| *puerto > 0)
                    .ok_or(ConfigError::Invalida("SMTP_PORT", valor))?,
                None => 587,
            },
            smtp_tls_enabled: match leer("SMTP_TLS_ENABLED") {
                Some(valor) if valor.eq_ignore_ascii_case("true") => true,
                Some(valor) if valor.eq_ignore_ascii_case("false") => false,
                Some(valor) => {
                    return Err(ConfigError::Invalida("SMTP_TLS_ENABLED", valor));
                }
                None => true,
            },
            smtp_user: leer("SMTP_USER"),
            smtp_password: leer("SMTP_PASSWORD"),
            exigir_correo_verificado: leer("AUTH_EXIGIR_CORREO_VERIFICADO")
                .map(|valor| valor.eq_ignore_ascii_case("true"))
                .unwrap_or(produccion),
            mfa_encryption_key: leer("MFA_ENCRYPTION_KEY"),
            servicios_purga: {
                let storage_url = leer("STORAGE_SERVICE_URL")
                    .unwrap_or_else(|| "http://storage-service:3010".to_string())
                    .trim_end_matches('/')
                    .to_string();
                let configurados = leer("TENANT_PURGE_SERVICES")
                    .map(|valor| ServicioPurga::parsear(&valor))
                    .unwrap_or_default();
                if configurados.is_empty() {
                    vec![ServicioPurga {
                        nombre: "storage".to_string(),
                        url: storage_url,
                    }]
                } else {
                    configurados
                }
            },
            auditoria_retencion_dias: leer("AUDITORIA_RETENCION_DIAS")
                .and_then(|valor| valor.parse::<i64>().ok())
                .filter(|dias| *dias >= 0)
                .unwrap_or(365),
            auditoria_retencion_modo: leer("AUDITORIA_RETENCION_MODO")
                .map(|valor| valor.to_lowercase())
                .filter(|valor| valor == "anonimizar" || valor == "eliminar")
                .unwrap_or_else(|| "anonimizar".to_string()),
            export_signing_key: leer("EXPORT_SIGNING_KEY"),
            plataforma_plan_codigo: leer("PLATAFORMA_PLAN_EMPRESARIO")
                .unwrap_or_else(|| "empresario_plus".to_string()),
            plataforma_empresas_gratis: leer("PLATAFORMA_EMPRESAS_GRATIS")
                .and_then(|valor| valor.parse::<i64>().ok())
                .filter(|valor| *valor >= 0)
                .unwrap_or(1),
            plataforma_negocios_gratis: leer("PLATAFORMA_NEGOCIOS_GRATIS")
                .and_then(|valor| valor.parse::<i64>().ok())
                .filter(|valor| *valor >= 0)
                .unwrap_or(1),
        })
    }

    /// `DATABASE_URL` es obligatoria para el arranque real (R-MS-5).
    pub fn validar_arranque(&self) -> Result<(), ConfigError> {
        if self.database_url.is_none() {
            return Err(ConfigError::Faltante("DATABASE_URL"));
        }
        if self.email_mode != "log" && self.email_mode != "smtp" {
            return Err(ConfigError::Invalida("EMAIL_MODE", self.email_mode.clone()));
        }
        if let Err(mensaje) = self.clave_mfa() {
            return Err(ConfigError::Invalida("MFA_ENCRYPTION_KEY", mensaje));
        }
        if self.email_mode == "smtp" {
            if self.smtp_host.is_none() {
                return Err(ConfigError::Faltante("SMTP_HOST"));
            }
            if self.email_from.is_none() {
                return Err(ConfigError::Faltante("EMAIL_FROM"));
            }
            if self.smtp_user.is_some() != self.smtp_password.is_some() {
                return Err(ConfigError::Invalida(
                    "SMTP_USER",
                    "SMTP_USER y SMTP_PASSWORD deben configurarse juntos.".to_string(),
                ));
            }
            if !self.smtp_tls_enabled
                && (self.produccion || self.smtp_user.is_some() || self.smtp_password.is_some())
            {
                return Err(ConfigError::Invalida(
                    "SMTP_TLS_ENABLED",
                    "SMTP sin TLS solo se permite en desarrollo y sin credenciales.".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Clave AES-256-GCM para secretos MFA. En producción debe venir en
    /// `MFA_ENCRYPTION_KEY` (base64 de 32 bytes); en desarrollo se deriva del
    /// JWT para no romper el arranque local.
    pub fn clave_mfa(&self) -> Result<[u8; 32], String> {
        if let Some(valor) = &self.mfa_encryption_key {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(valor.trim())
                .map_err(|_| "MFA_ENCRYPTION_KEY debe ser base64.".to_string())?;
            if bytes.len() != 32 {
                return Err("MFA_ENCRYPTION_KEY debe decodificar a 32 bytes.".to_string());
            }
            let mut clave = [0u8; 32];
            clave.copy_from_slice(&bytes);
            return Ok(clave);
        }
        if self.produccion {
            return Err("MFA_ENCRYPTION_KEY es obligatoria en produccion.".to_string());
        }
        let mut hasher = Sha256::new();
        hasher.update(self.jwt_secret.as_bytes());
        hasher.update(b"mfa-dev");
        let digest = hasher.finalize();
        let mut clave = [0u8; 32];
        clave.copy_from_slice(&digest);
        Ok(clave)
    }

    /// Convierte `JWT_ACCESS_TTL`/`JWT_REFRESH_TTL` (`900s`, `15m`, `7d`) a
    /// segundos, con el mismo fallback que `ttlAMs` (7 días).
    pub fn access_ttl_segundos(&self) -> u64 {
        ttl_a_segundos(&self.jwt_access_ttl)
    }

    pub fn refresh_ttl_segundos(&self) -> u64 {
        ttl_a_segundos(&self.jwt_refresh_ttl)
    }
}

/// Réplica de `ttlAMs` (`auth.service.ts`) pero en segundos.
pub fn ttl_a_segundos(ttl: &str) -> u64 {
    let ttl = ttl.trim();
    let (numero, unidad) = ttl.split_at(ttl.len().saturating_sub(1));
    let valor = numero.parse::<u64>().ok();
    let multiplicador = match unidad {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return 7 * 86_400,
    };
    valor
        .map(|valor| valor * multiplicador)
        .unwrap_or(7 * 86_400)
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
    fn debe_usar_defaults_de_desarrollo() {
        let config = Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config");
        assert_eq!(config.puerto, PUERTO_IDENTITY);
        assert_eq!(config.servicio, NOMBRE_SERVICIO);
        assert_eq!(config.outbox_tabla, "identity.outbox");
        assert_eq!(config.jwt_secret, JWT_SECRET_DEFAULT);
        assert_eq!(config.admin_email, ADMIN_EMAIL_DEFAULT);
        assert_eq!(config.jwt_access_ttl, "900s");
        assert!(!config.produccion);
        assert!(!config.exigir_correo_verificado);
        assert_eq!(config.plataforma_plan_codigo, "empresario_plus");
        assert_eq!(config.plataforma_empresas_gratis, 1);
        assert_eq!(config.plataforma_negocios_gratis, 1);
    }

    #[test]
    fn debe_configurar_la_regla_de_expansion_de_plataforma() {
        let config = Config::from_pairs([
            ("PLATAFORMA_PLAN_EMPRESARIO", "empresario_plus"),
            ("PLATAFORMA_EMPRESAS_GRATIS", "0"),
            ("PLATAFORMA_NEGOCIOS_GRATIS", "2"),
        ])
        .expect("config");
        assert_eq!(config.plataforma_empresas_gratis, 0);
        assert_eq!(config.plataforma_negocios_gratis, 2);

        let invalido = Config::from_pairs([("PLATAFORMA_NEGOCIOS_GRATIS", "-1")]).expect("config");
        assert_eq!(invalido.plataforma_negocios_gratis, 1);
    }

    #[test]
    fn debe_exigir_correo_verificado_en_produccion_o_por_flag() {
        let produccion = Config::from_pairs([
            ("NODE_ENV", "production"),
            ("JWT_SECRET", "s3creto"),
            ("ADMIN_EMAIL", "admin@test.dev"),
            ("ADMIN_PASSWORD", "AdminSeguro2026!"),
        ])
        .expect("config");
        assert!(produccion.exigir_correo_verificado);

        let explicito =
            Config::from_pairs([("AUTH_EXIGIR_CORREO_VERIFICADO", "true")]).expect("config");
        assert!(explicito.exigir_correo_verificado);
    }

    #[test]
    fn debe_leer_y_limpiar_las_variables() {
        let config = Config::from_pairs([
            ("PORT", "4001"),
            ("SERVICIO", " identity-service "),
            ("DATABASE_URL", "postgres://localhost/core"),
            ("JWT_SECRET", " s3creto "),
            ("FRONTEND_URL", "https://app.test/"),
            ("JWT_REFRESH_TTL", "30d"),
        ])
        .expect("config");
        assert_eq!(config.puerto, 4001);
        assert_eq!(config.jwt_secret, "s3creto");
        assert_eq!(config.frontend_url, "https://app.test");
        assert_eq!(config.refresh_ttl_segundos(), 30 * 86_400);
    }

    #[test]
    fn en_produccion_exige_los_secretos() {
        let error = Config::from_pairs([("NODE_ENV", "production")]).expect_err("jwt");
        assert_eq!(error, ConfigError::Faltante("JWT_SECRET"));
    }

    #[test]
    fn debe_rechazar_un_puerto_invalido() {
        let error = Config::from_pairs([("PORT", "abc")]).expect_err("puerto");
        assert_eq!(error, ConfigError::Invalida("PORT", "abc".to_string()));
    }

    #[test]
    fn debe_fallar_el_arranque_sin_database_url() {
        let config = Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Faltante("DATABASE_URL"))
        );
    }

    #[test]
    fn debe_exigir_configuracion_smtp_completa() {
        let config = Config::from_pairs([
            ("DATABASE_URL", "postgres://localhost/core"),
            ("EMAIL_MODE", "smtp"),
        ])
        .expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Faltante("SMTP_HOST"))
        );

        let config = Config::from_pairs([
            ("DATABASE_URL", "postgres://localhost/core"),
            ("EMAIL_MODE", "invalido"),
        ])
        .expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Invalida("EMAIL_MODE", "invalido".to_string()))
        );
    }

    #[test]
    fn smtp_sin_tls_solo_se_permite_en_desarrollo_sin_credenciales() {
        let local = Config::from_pairs([
            ("DATABASE_URL", "postgres://localhost/core"),
            ("EMAIL_MODE", "smtp"),
            ("EMAIL_FROM", "no-reply@example.test"),
            ("SMTP_HOST", "mailpit"),
            ("SMTP_PORT", "1025"),
            ("SMTP_TLS_ENABLED", "false"),
        ])
        .expect("config");
        assert!(local.validar_arranque().is_ok());

        let credenciales = Config::from_pairs([
            ("DATABASE_URL", "postgres://localhost/core"),
            ("EMAIL_MODE", "smtp"),
            ("EMAIL_FROM", "no-reply@example.test"),
            ("SMTP_HOST", "mailpit"),
            ("SMTP_TLS_ENABLED", "false"),
            ("SMTP_USER", "user"),
            ("SMTP_PASSWORD", "password"),
        ])
        .expect("config");
        assert!(matches!(
            credenciales.validar_arranque(),
            Err(ConfigError::Invalida("SMTP_TLS_ENABLED", _))
        ));

        let produccion = Config::from_pairs([
            ("NODE_ENV", "production"),
            ("JWT_SECRET", "jwt-secret"),
            ("ADMIN_EMAIL", "admin@example.test"),
            ("ADMIN_PASSWORD", "strong-password"),
            ("DATABASE_URL", "postgres://localhost/core"),
            ("EMAIL_MODE", "smtp"),
            ("EMAIL_FROM", "no-reply@example.test"),
            ("SMTP_HOST", "smtp.example.test"),
            ("SMTP_TLS_ENABLED", "false"),
            (
                "MFA_ENCRYPTION_KEY",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            ),
        ])
        .expect("config");
        assert!(matches!(
            produccion.validar_arranque(),
            Err(ConfigError::Invalida("SMTP_TLS_ENABLED", _))
        ));
    }

    #[test]
    fn debe_convertir_ttl_con_el_mismo_fallback() {
        assert_eq!(ttl_a_segundos("900s"), 900);
        assert_eq!(ttl_a_segundos("15m"), 900);
        assert_eq!(ttl_a_segundos("2h"), 7_200);
        assert_eq!(ttl_a_segundos("7d"), 604_800);
        assert_eq!(ttl_a_segundos("basura"), 604_800);
    }
}
