//! Configuración de runtime del servicio.
//!
//! Réplica de `@core/shared` (`PUERTOS`, `NOMBRE_SERVICIOS`) y de las reglas
//! R-MS-5: las variables críticas se validan al arrancar y el proceso falla
//! rápido si faltan.

use std::env;

pub const NOMBRE_SERVICIO: &str = "orders-service";
pub const PUERTO_ORDERS: u16 = 3004;
pub const TIMEOUT_INTERNO_DEFAULT_MS: u64 = 3000;
pub const TASA_COMISION_DEFAULT: f64 = 0.12;
pub const CARRITO_EXPIRACION_MS: i64 = 30 * 60 * 1000;
pub const ORDEN_PAGO_TTL_MINUTOS_DEFAULT: u64 = 30;
pub const ORDEN_EXPIRACION_INTERVALO_SEGUNDOS_DEFAULT: u64 = 60;
pub const PAGOS_CONCILIACION_INTERVALO_SEGUNDOS_DEFAULT: u64 = 900;
pub const PAGOS_CONCILIACION_MINUTOS_DEFAULT: u64 = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub puerto: u16,
    pub servicio: String,
    pub database_url: Option<String>,
    pub rabbitmq_url: Option<String>,
    pub internal_api_key: Option<String>,
    pub stores_service_url: String,
    pub item_service_url: String,
    pub pricing_service_url: String,
    pub inventory_service_url: String,
    pub identity_service_url: String,
    pub scheduling_service_url: String,
    pub finance_service_url: String,
    pub internal_request_timeout_ms: u64,
    pub outbox_tabla: String,
    pub tasa_comision: f64,
    pub pago_webhook_secret: Option<String>,
    pub pago_proveedor: String,
    pub pago_proveedor_url: Option<String>,
    pub pago_proveedor_key: Option<String>,
    /// Minutos que una orden puede quedar impaga antes de expirar. `0` desactiva
    /// el barrido.
    pub orden_pago_ttl_minutos: u64,
    pub orden_expiracion_intervalo_segundos: u64,
    /// Cada cuántos segundos se consultan los pagos pendientes con pasarela.
    pub pagos_conciliacion_intervalo_segundos: u64,
    /// Ventana de gracia antes de consultar un pago pendiente.
    pub pagos_conciliacion_minutos: u64,
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
            None => PUERTO_ORDERS,
        };

        Ok(Self {
            puerto,
            servicio: leer("SERVICIO").unwrap_or_else(|| NOMBRE_SERVICIO.to_string()),
            database_url: leer("DATABASE_URL"),
            rabbitmq_url: leer("RABBITMQ_URL"),
            internal_api_key: leer("INTERNAL_API_KEY"),
            stores_service_url: url_con_default(
                &leer,
                "STORES_SERVICE_URL",
                "http://stores-service:3003",
            ),
            item_service_url: url_con_default(
                &leer,
                "ITEM_SERVICE_URL",
                "http://item-service:3015",
            ),
            pricing_service_url: url_con_default(
                &leer,
                "PRICING_SERVICE_URL",
                "http://pricing-service:3014",
            ),
            inventory_service_url: url_con_default(
                &leer,
                "INVENTORY_SERVICE_URL",
                "http://inventory-service:3013",
            ),
            identity_service_url: url_con_default(
                &leer,
                "IDENTITY_SERVICE_URL",
                "http://identity-service:3001",
            ),
            scheduling_service_url: url_con_default(
                &leer,
                "SCHEDULING_SERVICE_URL",
                "http://scheduling-service:3011",
            ),
            finance_service_url: url_con_default(
                &leer,
                "FINANCE_SERVICE_URL",
                "http://finance-service:3007",
            ),
            internal_request_timeout_ms: match leer("INTERNAL_REQUEST_TIMEOUT_MS") {
                Some(valor) => valor
                    .parse::<u64>()
                    .ok()
                    .filter(|numero| *numero > 0)
                    .unwrap_or(TIMEOUT_INTERNO_DEFAULT_MS),
                None => TIMEOUT_INTERNO_DEFAULT_MS,
            },
            outbox_tabla: leer("OUTBOX_TABLA").unwrap_or_else(|| "orders.outbox".to_string()),
            pago_webhook_secret: leer("PAGO_WEBHOOK_SECRET"),
            pago_proveedor: leer("PAGO_PROVEEDOR").unwrap_or_else(|| "simulado".to_string()),
            pago_proveedor_url: leer("PAGO_PROVEEDOR_URL"),
            pago_proveedor_key: leer("PAGO_PROVEEDOR_KEY"),
            orden_pago_ttl_minutos: leer("ORDEN_PAGO_TTL_MINUTOS")
                .and_then(|valor| valor.parse::<u64>().ok())
                .unwrap_or(ORDEN_PAGO_TTL_MINUTOS_DEFAULT),
            orden_expiracion_intervalo_segundos: leer("ORDEN_EXPIRACION_INTERVALO_SEGUNDOS")
                .and_then(|valor| valor.parse::<u64>().ok())
                .filter(|segundos| *segundos > 0)
                .unwrap_or(ORDEN_EXPIRACION_INTERVALO_SEGUNDOS_DEFAULT),
            pagos_conciliacion_intervalo_segundos: leer("PAGOS_CONCILIACION_INTERVALO_SEGUNDOS")
                .and_then(|valor| valor.parse::<u64>().ok())
                .filter(|segundos| *segundos > 0)
                .unwrap_or(PAGOS_CONCILIACION_INTERVALO_SEGUNDOS_DEFAULT),
            pagos_conciliacion_minutos: leer("PAGOS_CONCILIACION_MINUTOS")
                .and_then(|valor| valor.parse::<u64>().ok())
                .unwrap_or(PAGOS_CONCILIACION_MINUTOS_DEFAULT),
            tasa_comision: match leer("COMMISSION_RATE") {
                Some(valor) => valor
                    .parse::<f64>()
                    .ok()
                    .filter(|tasa| tasa.is_finite() && *tasa >= 0.0 && *tasa < 1.0)
                    .unwrap_or(TASA_COMISION_DEFAULT),
                None => TASA_COMISION_DEFAULT,
            },
        })
    }

    pub fn validar_arranque(&self) -> Result<(), ConfigError> {
        if self.database_url.is_none() {
            return Err(ConfigError::Faltante("DATABASE_URL"));
        }
        if self.rabbitmq_url.is_none() {
            return Err(ConfigError::Faltante("RABBITMQ_URL"));
        }
        if self.pago_proveedor.eq_ignore_ascii_case("http") && self.pago_proveedor_url.is_none() {
            return Err(ConfigError::Faltante("PAGO_PROVEEDOR_URL"));
        }
        Ok(())
    }
}

fn url_con_default(
    leer: &impl Fn(&str) -> Option<String>,
    clave: &str,
    por_defecto: &str,
) -> String {
    leer(clave)
        .unwrap_or_else(|| por_defecto.to_string())
        .trim_end_matches('/')
        .to_string()
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
        assert_eq!(config.puerto, PUERTO_ORDERS);
        assert_eq!(config.servicio, NOMBRE_SERVICIO);
        assert_eq!(config.outbox_tabla, "orders.outbox");
        assert_eq!(config.tasa_comision, TASA_COMISION_DEFAULT);
        assert_eq!(
            config.orden_pago_ttl_minutos,
            ORDEN_PAGO_TTL_MINUTOS_DEFAULT
        );
        assert_eq!(
            config.orden_expiracion_intervalo_segundos,
            ORDEN_EXPIRACION_INTERVALO_SEGUNDOS_DEFAULT
        );
        assert_eq!(config.stores_service_url, "http://stores-service:3003");
        assert_eq!(
            config.inventory_service_url,
            "http://inventory-service:3013"
        );
        assert_eq!(config.database_url, None);
    }

    #[test]
    fn debe_leer_y_limpiar_las_variables_de_entorno() {
        let config = Config::from_pairs([
            ("PORT", "4004"),
            ("DATABASE_URL", "postgres://localhost/core"),
            ("RABBITMQ_URL", "amqp://localhost"),
            ("STORES_SERVICE_URL", "http://stores:3003/"),
            ("ITEM_SERVICE_URL", "http://item:3015/"),
            ("PRICING_SERVICE_URL", "http://pricing:3014/"),
            ("INVENTORY_SERVICE_URL", "http://inventory:3013/"),
            ("IDENTITY_SERVICE_URL", "http://identity:3001/"),
            ("SCHEDULING_SERVICE_URL", "http://scheduling:3011/"),
            ("FINANCE_SERVICE_URL", "http://finance:3007/"),
            ("INTERNAL_REQUEST_TIMEOUT_MS", "1500"),
            ("OUTBOX_TABLA", "orders.outbox"),
            ("COMMISSION_RATE", "0.1"),
            ("ORDEN_PAGO_TTL_MINUTOS", "45"),
            ("ORDEN_EXPIRACION_INTERVALO_SEGUNDOS", "120"),
        ])
        .expect("config");
        assert_eq!(config.puerto, 4004);
        assert_eq!(config.orden_pago_ttl_minutos, 45);
        assert_eq!(config.orden_expiracion_intervalo_segundos, 120);
        assert_eq!(config.stores_service_url, "http://stores:3003");
        assert_eq!(config.item_service_url, "http://item:3015");
        assert_eq!(config.pricing_service_url, "http://pricing:3014");
        assert_eq!(config.inventory_service_url, "http://inventory:3013");
        assert_eq!(config.identity_service_url, "http://identity:3001");
        assert_eq!(config.scheduling_service_url, "http://scheduling:3011");
        assert_eq!(config.finance_service_url, "http://finance:3007");
        assert_eq!(config.internal_request_timeout_ms, 1500);
        assert_eq!(config.tasa_comision, 0.1);
    }

    #[test]
    fn debe_rechazar_un_puerto_invalido() {
        let error = Config::from_pairs([("PORT", "abc")]).expect_err("puerto invalido");
        assert_eq!(error, ConfigError::Invalida("PORT", "abc".to_string()));
    }

    #[test]
    fn debe_descartar_timeouts_y_tasas_invalidas() {
        let config = Config::from_pairs([
            ("INTERNAL_REQUEST_TIMEOUT_MS", "0"),
            ("COMMISSION_RATE", "1.5"),
        ])
        .expect("config");
        assert_eq!(
            config.internal_request_timeout_ms,
            TIMEOUT_INTERNO_DEFAULT_MS
        );
        assert_eq!(config.tasa_comision, TASA_COMISION_DEFAULT);
    }

    #[test]
    fn debe_aceptar_una_tasa_cero_para_venta_directa() {
        let config = Config::from_pairs([("COMMISSION_RATE", "0")]).expect("config");
        assert_eq!(config.tasa_comision, 0.0);
    }

    #[test]
    fn debe_permitir_desactivar_la_expiracion_y_descartar_intervalos_invalidos() {
        let config = Config::from_pairs([
            ("ORDEN_PAGO_TTL_MINUTOS", "0"),
            ("ORDEN_EXPIRACION_INTERVALO_SEGUNDOS", "0"),
        ])
        .expect("config");
        assert_eq!(config.orden_pago_ttl_minutos, 0);
        assert_eq!(
            config.orden_expiracion_intervalo_segundos,
            ORDEN_EXPIRACION_INTERVALO_SEGUNDOS_DEFAULT
        );
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
