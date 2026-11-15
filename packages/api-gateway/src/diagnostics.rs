//! Cierre de la superficie de diagnóstico (ADR-0005, R-GW-8).

use crate::config::Config;
use axum::http::HeaderMap;

pub fn es_ruta_diagnostico(ruta: &str) -> bool {
    ruta.starts_with("/docs") || ruta == "/metrics"
}

pub fn autorizado(headers: &HeaderMap, ruta: &str, config: &Config) -> bool {
    if ruta == "/health" {
        return true;
    }
    let clave = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok());
    let clave_valida = match (clave, config.internal_api_key.as_deref()) {
        (Some(recibida), Some(esperada)) => iguales(recibida, esperada),
        _ => false,
    };
    if ruta == "/ready" {
        return config.ready_publico || clave_valida;
    }
    if es_ruta_diagnostico(ruta) {
        return config.diagnosticos_publicos || clave_valida;
    }
    true
}

/// Comparación en tiempo constante para no filtrar la credencial por prefijo.
pub fn iguales(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diferencia = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diferencia |= x ^ y;
    }
    diferencia == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config::from_pairs([("INTERNAL_API_KEY", "k")]).expect("config")
    }

    #[test]
    fn debe_abrir_solo_health_y_exigir_clave() {
        let config = config();
        assert!(autorizado(&HeaderMap::new(), "/health", &config));
        assert!(!autorizado(&HeaderMap::new(), "/ready", &config));
        assert!(!autorizado(&HeaderMap::new(), "/metrics", &config));
        let mut headers = HeaderMap::new();
        headers.insert("x-internal-key", "k".parse().expect("header"));
        assert!(autorizado(&headers, "/ready", &config));
        assert!(autorizado(&headers, "/docs", &config));
    }

    #[test]
    fn debe_permitir_diagnostico_publico_en_desarrollo() {
        let config = Config::from_pairs([("DIAGNOSTICOS_PUBLICOS", "true")]).expect("config");
        assert!(autorizado(&HeaderMap::new(), "/docs", &config));
        assert!(!autorizado(&HeaderMap::new(), "/ready", &config));
    }
}
