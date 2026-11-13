//! Configuración del gateway (puerto, secreto JWT, destinos y diagnóstico).

use std::collections::HashMap;
use std::env;

pub const NOMBRE_SERVICIO: &str = "api-gateway";
pub const PUERTO_GATEWAY: u16 = 3000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub puerto: u16,
    pub servicio: String,
    pub jwt_secret: String,
    pub internal_api_key: Option<String>,
    pub cors_origins: Vec<String>,
    pub diagnosticos_publicos: bool,
    pub ready_publico: bool,
    pub entorno: String,
    pub proxy_timeout_ms: u64,
    pub ready_cache_ms: u64,
    pub identity_url: String,
    pub storage_url: String,
    pub catalog_url: String,
    pub stores_url: String,
    pub scheduling_url: String,
    pub logistics_url: String,
    pub orders_url: String,
    pub commissions_url: String,
    pub finance_url: String,
    pub field_url: String,
    pub intelligence_url: String,
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
        let jwt_secret = match leer("JWT_SECRET") {
            Some(valor) => valor,
            None if produccion => return Err(ConfigError::Faltante("JWT_SECRET")),
            None => "dev_secret".to_string(),
        };
        let puerto = match leer("PORT") {
            Some(valor) => valor
                .parse::<u16>()
                .map_err(|_| ConfigError::Invalida("PORT", valor))?,
            None => PUERTO_GATEWAY,
        };
        let proxy_timeout_ms = entero_positivo(leer("PROXY_UPSTREAM_TIMEOUT_MS"), 30_000)?;
        let ready_cache_ms = entero_positivo(leer("READY_CACHE_MS"), 5_000)?;
        Ok(Self {
            puerto,
            servicio: leer("SERVICIO").unwrap_or_else(|| NOMBRE_SERVICIO.to_string()),
            jwt_secret,
            internal_api_key: leer("INTERNAL_API_KEY"),
            cors_origins: leer("CORS_ORIGINS")
                .map(|valor| {
                    valor
                        .split(',')
                        .map(|o| o.trim().to_string())
                        .filter(|o| !o.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            diagnosticos_publicos: leer("DIAGNOSTICOS_PUBLICOS")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            ready_publico: leer("READY_PUBLIC")
                .map(|v| v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            entorno: leer("ENTORNO").unwrap_or_default(),
            proxy_timeout_ms,
            ready_cache_ms,
            identity_url: url_servicio(&leer, "IDENTITY_SERVICE_URL", "identity", 3001),
            storage_url: url_servicio(&leer, "STORAGE_SERVICE_URL", "storage", 3010),
            catalog_url: url_servicio(&leer, "CATALOG_SERVICE_URL", "catalog", 3002),
            stores_url: url_servicio(&leer, "STORES_SERVICE_URL", "stores", 3003),
            scheduling_url: url_servicio(&leer, "SCHEDULING_SERVICE_URL", "scheduling", 3011),
            logistics_url: url_servicio(&leer, "LOGISTICS_SERVICE_URL", "logistics", 3005),
            orders_url: url_servicio(&leer, "ORDERS_SERVICE_URL", "orders", 3004),
            commissions_url: url_servicio(&leer, "COMMISSIONS_SERVICE_URL", "commissions", 3006),
            finance_url: url_servicio(&leer, "FINANCE_SERVICE_URL", "finance", 3007),
            field_url: url_servicio(&leer, "FIELD_SERVICE_URL", "field", 3008),
            intelligence_url: url_servicio(
                &leer,
                "INTELLIGENCE_SERVICE_URL",
                "market-intelligence",
                3009,
            ),
        })
    }
}

fn url_servicio(
    leer: &impl Fn(&str) -> Option<String>,
    variable: &str,
    nombre: &str,
    puerto: u16,
) -> String {
    leer(variable)
        .unwrap_or_else(|| format!("http://{nombre}-service:{puerto}"))
        .trim_end_matches('/')
        .to_string()
}

fn entero_positivo(valor: Option<String>, por_defecto: u64) -> Result<u64, ConfigError> {
    match valor {
        Some(valor) => valor
            .parse::<u64>()
            .ok()
            .filter(|numero| *numero > 0)
            .ok_or(ConfigError::Invalida("numero", valor)),
        None => Ok(por_defecto),
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{0} es obligatoria en produccion.")]
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
        assert_eq!(config.puerto, PUERTO_GATEWAY);
        assert_eq!(config.jwt_secret, "dev_secret");
        assert_eq!(config.identity_url, "http://identity-service:3001");
        assert_eq!(config.proxy_timeout_ms, 30_000);
        assert_eq!(config.ready_cache_ms, 5_000);
    }

    #[test]
    fn debe_exigir_jwt_en_produccion() {
        assert_eq!(
            Config::from_pairs([("NODE_ENV", "production")]).expect_err("jwt"),
            ConfigError::Faltante("JWT_SECRET")
        );
    }

    #[test]
    fn debe_leer_urls_y_cors() {
        let config = Config::from_pairs([
            ("CORS_ORIGINS", "https://a.test, https://b.test"),
            ("IDENTITY_SERVICE_URL", "http://identity:4001/"),
            ("ENTORNO", "stage"),
        ])
        .expect("config");
        assert_eq!(
            config.cors_origins,
            vec!["https://a.test", "https://b.test"]
        );
        assert_eq!(config.identity_url, "http://identity:4001");
        assert_eq!(config.entorno, "stage");
    }
}
