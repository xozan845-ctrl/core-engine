//! Configuración de runtime del storage-service.
//!
//! Réplica de `config/storage.config.ts` y de las reglas R-MS-5: las variables
//! críticas se validan al arrancar y el proceso falla rápido si faltan.

use std::collections::HashMap;
use std::env;

pub const NOMBRE_SERVICIO: &str = "storage-service";
pub const PUERTO_STORAGE: u16 = 3010;

pub const BUCKET_PREFIX_DEFAULT: &str = "t-";
pub const MAX_OBJETO_BYTES_DEFAULT: i64 = 25 * 1024 * 1024;
pub const MAX_VIDEO_BYTES_DEFAULT: i64 = 100 * 1024 * 1024;
pub const CUOTA_DEFAULT_BYTES: i64 = 5 * 1024 * 1024 * 1024;
pub const URL_TTL_SEGUNDOS_DEFAULT: u64 = 900;

pub const CONTENT_TYPES_DEFAULT: [&str; 10] = [
    "image/png",
    "image/jpeg",
    "image/webp",
    "video/mp4",
    "video/webm",
    "application/pdf",
    "text/csv",
    "text/plain",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/json",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub puerto: u16,
    pub servicio: String,
    pub database_url: Option<String>,
    pub rabbitmq_url: Option<String>,
    pub internal_api_key: Option<String>,
    pub outbox_tabla: String,
    pub platform_tenant_id: Option<String>,
    pub minio_endpoint: String,
    pub minio_port: u16,
    pub minio_use_ssl: bool,
    pub minio_public_endpoint: String,
    pub minio_public_port: u16,
    pub minio_public_use_ssl: bool,
    pub minio_access_key: Option<String>,
    pub minio_secret_key: Option<String>,
    pub minio_region: String,
    pub bucket_prefix: String,
    pub max_objeto_bytes: i64,
    pub max_video_bytes: i64,
    pub cuota_default_bytes: i64,
    pub url_ttl_segundos: u64,
    pub content_types_permitidos: Vec<String>,
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

        let puerto = match leer("PORT") {
            Some(valor) => valor
                .parse::<u16>()
                .map_err(|_| ConfigError::Invalida("PORT", valor))?,
            None => PUERTO_STORAGE,
        };
        let minio_use_ssl = leer("MINIO_USE_SSL")
            .map(|valor| valor.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let public_use_ssl = leer("MINIO_PUBLIC_USE_SSL")
            .map(|valor| valor.eq_ignore_ascii_case("true"))
            .unwrap_or(minio_use_ssl);
        let minio_port = entero_positivo(leer("MINIO_PORT"), 9000, "MINIO_PORT")?;
        let public_port =
            entero_positivo(leer("MINIO_PUBLIC_PORT"), minio_port, "MINIO_PUBLIC_PORT")?;

        let content_types = match leer("STORAGE_CONTENT_TYPES_PERMITIDOS") {
            Some(valor) => valor
                .split(',')
                .map(|item| item.trim().to_lowercase())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>(),
            None => CONTENT_TYPES_DEFAULT
                .iter()
                .map(|valor| (*valor).to_string())
                .collect(),
        };

        Ok(Self {
            puerto,
            servicio: leer("SERVICIO").unwrap_or_else(|| NOMBRE_SERVICIO.to_string()),
            database_url: leer("DATABASE_URL"),
            rabbitmq_url: leer("RABBITMQ_URL"),
            internal_api_key: leer("INTERNAL_API_KEY"),
            outbox_tabla: leer("OUTBOX_TABLA").unwrap_or_else(|| "storage.outbox".to_string()),
            platform_tenant_id: leer("PLATFORM_TENANT_ID"),
            minio_endpoint: leer("MINIO_ENDPOINT").unwrap_or_else(|| "minio".to_string()),
            minio_port: minio_port as u16,
            minio_use_ssl,
            minio_public_endpoint: leer("MINIO_PUBLIC_ENDPOINT")
                .unwrap_or_else(|| leer("MINIO_ENDPOINT").unwrap_or_else(|| "minio".to_string())),
            minio_public_port: public_port as u16,
            minio_public_use_ssl: public_use_ssl,
            minio_access_key: leer("MINIO_ROOT_USER"),
            minio_secret_key: leer("MINIO_ROOT_PASSWORD"),
            minio_region: leer("MINIO_REGION").unwrap_or_else(|| "us-east-1".to_string()),
            bucket_prefix: leer("STORAGE_BUCKET_PREFIX")
                .unwrap_or_else(|| BUCKET_PREFIX_DEFAULT.to_string()),
            max_objeto_bytes: entero_positivo(
                leer("STORAGE_MAX_OBJETO_BYTES"),
                MAX_OBJETO_BYTES_DEFAULT,
                "STORAGE_MAX_OBJETO_BYTES",
            )?,
            max_video_bytes: entero_positivo(
                leer("STORAGE_MAX_VIDEO_BYTES"),
                MAX_VIDEO_BYTES_DEFAULT,
                "STORAGE_MAX_VIDEO_BYTES",
            )?,
            cuota_default_bytes: entero_positivo(
                leer("STORAGE_CUOTA_DEFAULT_BYTES").or_else(|| leer("STORAGE_CUOTA_DEFAULT")),
                CUOTA_DEFAULT_BYTES,
                "STORAGE_CUOTA_DEFAULT_BYTES",
            )?,
            url_ttl_segundos: entero_positivo(
                leer("STORAGE_URL_TTL_SEGUNDOS"),
                URL_TTL_SEGUNDOS_DEFAULT as i64,
                "STORAGE_URL_TTL_SEGUNDOS",
            )? as u64,
            content_types_permitidos: content_types,
        })
    }

    /// URL completa del endpoint S3 interno (sin barra final).
    pub fn endpoint_url(&self) -> String {
        url_endpoint(&self.minio_endpoint, self.minio_port, self.minio_use_ssl)
    }

    /// URL que se firma en las URLs prefirmadas (host público).
    pub fn public_endpoint_url(&self) -> String {
        url_endpoint(
            &self.minio_public_endpoint,
            self.minio_public_port,
            self.minio_public_use_ssl,
        )
    }

    pub fn validar_arranque(&self) -> Result<(), ConfigError> {
        if self.database_url.is_none() {
            return Err(ConfigError::Faltante("DATABASE_URL"));
        }
        if self.rabbitmq_url.is_none() {
            return Err(ConfigError::Faltante("RABBITMQ_URL"));
        }
        if self.minio_access_key.is_none() {
            return Err(ConfigError::Faltante("MINIO_ROOT_USER"));
        }
        if self.minio_secret_key.is_none() {
            return Err(ConfigError::Faltante("MINIO_ROOT_PASSWORD"));
        }
        Ok(())
    }

    pub fn maximo_para(&self, content_type: &str) -> i64 {
        if content_type.starts_with("video/") {
            self.max_video_bytes
        } else {
            self.max_objeto_bytes
        }
    }
}

fn url_endpoint(host: &str, puerto: u16, ssl: bool) -> String {
    let esquema = if ssl { "https" } else { "http" };
    format!("{esquema}://{host}:{puerto}")
}

fn entero_positivo(
    valor: Option<String>,
    por_defecto: i64,
    campo: &'static str,
) -> Result<i64, ConfigError> {
    match valor {
        Some(valor) => valor
            .parse::<i64>()
            .ok()
            .filter(|numero| *numero > 0)
            .ok_or(ConfigError::Invalida(campo, valor)),
        None => Ok(por_defecto),
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
    fn debe_usar_defaults_seguros() {
        let config = Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config");
        assert_eq!(config.puerto, PUERTO_STORAGE);
        assert_eq!(config.outbox_tabla, "storage.outbox");
        assert_eq!(config.bucket_prefix, "t-");
        assert_eq!(config.endpoint_url(), "http://minio:9000");
        assert_eq!(config.maximo_para("video/mp4"), MAX_VIDEO_BYTES_DEFAULT);
        assert_eq!(config.maximo_para("image/png"), MAX_OBJETO_BYTES_DEFAULT);
        assert!(config
            .content_types_permitidos
            .contains(&"image/webp".to_string()));
    }

    #[test]
    fn debe_leer_limites_y_endpoint_publico() {
        let config = Config::from_pairs([
            ("PORT", "4010"),
            ("MINIO_ENDPOINT", "minio"),
            ("MINIO_PORT", "9000"),
            ("MINIO_PUBLIC_ENDPOINT", "media.example.com"),
            ("MINIO_PUBLIC_PORT", "443"),
            ("MINIO_PUBLIC_USE_SSL", "true"),
            ("STORAGE_MAX_OBJETO_BYTES", "1024"),
            ("STORAGE_MAX_VIDEO_BYTES", "2048"),
            ("STORAGE_URL_TTL_SEGUNDOS", "60"),
        ])
        .expect("config");
        assert_eq!(config.puerto, 4010);
        assert_eq!(
            config.public_endpoint_url(),
            "https://media.example.com:443"
        );
        assert_eq!(config.max_objeto_bytes, 1024);
        assert_eq!(config.max_video_bytes, 2048);
        assert_eq!(config.url_ttl_segundos, 60);
    }

    #[test]
    fn debe_rechazar_puerto_o_limite_invalido() {
        assert_eq!(
            Config::from_pairs([("PORT", "abc")]).expect_err("puerto"),
            ConfigError::Invalida("PORT", "abc".to_string())
        );
        assert_eq!(
            Config::from_pairs([("STORAGE_MAX_OBJETO_BYTES", "0")]).expect_err("limite"),
            ConfigError::Invalida("STORAGE_MAX_OBJETO_BYTES", "0".to_string())
        );
    }

    #[test]
    fn debe_fallar_sin_dependencias_criticas() {
        let config = Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config");
        assert_eq!(
            config.validar_arranque(),
            Err(ConfigError::Faltante("DATABASE_URL"))
        );
    }
}
