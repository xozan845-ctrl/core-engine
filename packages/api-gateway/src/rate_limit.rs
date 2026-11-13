//! Rate limit en memoria por IP y cubo (global 300/min, login 10/min).

use axum::http::HeaderMap;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct RateLimit {
    ventanas: Mutex<HashMap<String, (Instant, u32)>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultadoLimite {
    Permitido { restantes: u32 },
    Excedido { reintento_seg: u64, limite: u32 },
}

impl RateLimit {
    pub fn nueva() -> Self {
        Self {
            ventanas: Mutex::new(HashMap::new()),
        }
    }

    pub fn consumir(&self, ip: &str, clave: &str, limite: u32, ttl: Duration) -> ResultadoLimite {
        let mut ventanas = self.ventanas.lock().expect("rate limit");
        let ahora = Instant::now();
        let indice = format!("{clave}:{ip}");
        let entrada = ventanas.entry(indice).or_insert((ahora, 0));
        if ahora.duration_since(entrada.0) >= ttl {
            *entrada = (ahora, 1);
        } else {
            entrada.1 += 1;
        }
        if entrada.1 > limite {
            let restante = ttl.saturating_sub(ahora.duration_since(entrada.0));
            ResultadoLimite::Excedido {
                reintento_seg: restante.as_secs().max(1),
                limite,
            }
        } else {
            ResultadoLimite::Permitido {
                restantes: limite - entrada.1,
            }
        }
    }
}

pub fn ip_de(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|valor| valor.to_str().ok())
        .and_then(|valor| valor.split(',').next())
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|valor| valor.to_str().ok())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "desconocida".to_string())
}

pub fn es_media_publica(metodo: &str, ruta: &str) -> bool {
    const PREFIJO: &str = "/api/v2/storage/public/objetos/";
    matches!(metodo, "GET" | "HEAD")
        && ruta
            .strip_prefix(PREFIJO)
            .map(|resto| !resto.is_empty() && !resto.contains('/'))
            .unwrap_or(false)
}

pub fn es_login(metodo: &str, ruta: &str) -> bool {
    metodo == "POST" && matches!(ruta, "/api/v2/auth/login" | "/api/v2/auth/token")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_permitir_y_luego_exceder_el_limite() {
        let rate = RateLimit::nueva();
        let ttl = Duration::from_secs(60);
        assert!(matches!(
            rate.consumir("1.1.1.1", "global", 2, ttl),
            ResultadoLimite::Permitido { restantes: 1 }
        ));
        assert!(matches!(
            rate.consumir("1.1.1.1", "global", 2, ttl),
            ResultadoLimite::Permitido { restantes: 0 }
        ));
        assert!(matches!(
            rate.consumir("1.1.1.1", "global", 2, ttl),
            ResultadoLimite::Excedido { .. }
        ));
        assert!(matches!(
            rate.consumir("2.2.2.2", "global", 2, ttl),
            ResultadoLimite::Permitido { .. }
        ));
    }

    #[test]
    fn debe_clasificar_rutas() {
        assert!(es_media_publica("GET", "/api/v2/storage/public/objetos/x"));
        assert!(!es_media_publica(
            "POST",
            "/api/v2/storage/public/objetos/x"
        ));
        assert!(!es_media_publica(
            "GET",
            "/api/v2/storage/public/objetos/x/y"
        ));
        assert!(es_login("POST", "/api/v2/auth/login"));
        assert!(!es_login("POST", "/api/v2/auth/logout"));
    }
}
