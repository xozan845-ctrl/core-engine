//! Métricas Prometheus con los mismos nombres y etiquetas que `MetricsService`.
//!
//! Se implementa un registro mínimo (contadores + histograma) para no arrastrar
//! una dependencia externa en el andamiaje; los nombres y el formato de texto
//! son los del contrato de observabilidad.

use chrono::{SecondsFormat, Utc};
use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Instant;

const BUCKETS: [f64; 7] = [0.05, 0.1, 0.2, 0.4, 0.8, 1.6, 3.2];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Etiquetas {
    estado: String,
    metodo: String,
    ruta: String,
}

#[derive(Debug, Default, Clone)]
struct Histograma {
    buckets: [u64; 7],
    suma: f64,
    total: u64,
}

#[derive(Default)]
struct Inner {
    peticiones: BTreeMap<Etiquetas, u64>,
    errores_servidor: BTreeMap<Etiquetas, u64>,
    errores_cliente: BTreeMap<Etiquetas, u64>,
    latencia: BTreeMap<Etiquetas, Histograma>,
    contadores: BTreeMap<String, u64>,
    gauges: BTreeMap<String, f64>,
}

pub struct Metrics {
    servicio: String,
    inicio: Instant,
    inner: Mutex<Inner>,
}

#[derive(Debug, serde::Serialize, PartialEq, Eq)]
pub struct MetricaSalud {
    pub servicio: String,
    pub uptime_seg: u64,
}

impl Metrics {
    pub fn nueva(servicio: impl Into<String>) -> Self {
        Self {
            servicio: servicio.into(),
            inicio: Instant::now(),
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Registra una petición HTTP. `ruta` debe ser el patrón declarado (no la
    /// URL cruda) para mantener la cardinalidad acotada.
    pub fn registrar_peticion(&self, metodo: &str, ruta: &str, status: u16, duracion_ms: u64) {
        let etiquetas = Etiquetas {
            estado: if (100..=599).contains(&status) {
                status.to_string()
            } else {
                "desconocido".to_string()
            },
            metodo: metodo.to_uppercase(),
            ruta: parametrizar_ruta(ruta),
        };
        let segundos = duracion_ms as f64 / 1000.0;
        let mut inner = self.inner.lock().expect("metricas");
        *inner.peticiones.entry(etiquetas.clone()).or_insert(0) += 1;
        if (500..=599).contains(&status) {
            *inner.errores_servidor.entry(etiquetas.clone()).or_insert(0) += 1;
        }
        if matches!(status, 401 | 403 | 409) {
            *inner.errores_cliente.entry(etiquetas.clone()).or_insert(0) += 1;
        }
        let histograma = inner.latencia.entry(etiquetas).or_default();
        for (indice, limite) in BUCKETS.iter().enumerate() {
            if segundos <= *limite {
                histograma.buckets[indice] += 1;
            }
        }
        histograma.suma += segundos;
        histograma.total += 1;
    }

    /// Contador de negocio acumulativo (`core_engine_<subsistema>_<nombre>_total`).
    pub fn incrementar_contador(&self, nombre: &str) {
        assert!(
            nombre.starts_with("core_engine_")
                && nombre.ends_with("_total")
                && nombre.matches('_').count() >= 3,
            "Métrica inválida: {nombre}. Usa core_engine_<subsistema>_<nombre>_total."
        );
        let mut inner = self.inner.lock().expect("metricas");
        *inner.contadores.entry(nombre.to_string()).or_insert(0) += 1;
    }

    /// Gauge de negocio (`core_engine_<subsistema>_<nombre>_<unidad>`).
    pub fn registrar_gauge(&self, nombre: &str, valor: f64) {
        assert!(
            nombre.starts_with("core_engine_")
                && !nombre.ends_with("_total")
                && nombre.matches('_').count() >= 3,
            "Métrica inválida: {nombre}. Usa core_engine_<subsistema>_<nombre>_<unidad>."
        );
        let mut inner = self.inner.lock().expect("metricas");
        inner.gauges.insert(nombre.to_string(), valor);
    }

    pub fn salud(&self) -> MetricaSalud {
        MetricaSalud {
            servicio: self.servicio.clone(),
            uptime_seg: self.inicio.elapsed().as_secs(),
        }
    }

    /// Texto en formato de exposición Prometheus (equivalente a `registry.metrics()`).
    pub fn texto(&self) -> String {
        let inner = self.inner.lock().expect("metricas");
        let mut salida = String::new();
        salida.push_str(&contador(
            "core_engine_http_requests_total",
            "Total de peticiones HTTP",
            &inner.peticiones,
            &self.servicio,
        ));
        salida.push_str(&contador(
            "core_engine_http_server_errors_total",
            "Total de respuestas HTTP 5xx",
            &inner.errores_servidor,
            &self.servicio,
        ));
        salida.push_str(&contador(
            "core_engine_http_relevant_client_errors_total",
            "Total de respuestas HTTP 401, 403 y 409",
            &inner.errores_cliente,
            &self.servicio,
        ));
        salida.push_str(&histograma_texto(&inner.latencia, &self.servicio));
        for (nombre, valor) in &inner.contadores {
            salida.push_str(&format!(
                "# HELP {nombre} Counter de negocio: {nombre}\n# TYPE {nombre} counter\n{nombre} {valor}\n"
            ));
        }
        for (nombre, valor) in &inner.gauges {
            salida.push_str(&format!(
                "# HELP {nombre} Gauge de negocio: {nombre}\n# TYPE {nombre} gauge\n{nombre} {valor}\n"
            ));
        }
        salida
    }
}

fn escapar(valor: &str) -> String {
    valor
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn etiquetas_texto(etiquetas: &Etiquetas, servicio: &str, extra: Option<(&str, &str)>) -> String {
    let mut partes = vec![
        format!("estado=\"{}\"", escapar(&etiquetas.estado)),
        format!("metodo=\"{}\"", escapar(&etiquetas.metodo)),
        format!("ruta=\"{}\"", escapar(&etiquetas.ruta)),
        format!("servicio=\"{}\"", escapar(servicio)),
    ];
    if let Some((clave, valor)) = extra {
        partes.push(format!("{}=\"{}\"", clave, escapar(valor)));
    }
    partes.join(",")
}

fn contador(nombre: &str, ayuda: &str, datos: &BTreeMap<Etiquetas, u64>, servicio: &str) -> String {
    let mut salida = format!("# HELP {nombre} {ayuda}\n# TYPE {nombre} counter\n");
    for (etiquetas, valor) in datos {
        salida.push_str(&format!(
            "{nombre}{{{}}} {valor}\n",
            etiquetas_texto(etiquetas, servicio, None)
        ));
    }
    salida
}

fn histograma_texto(datos: &BTreeMap<Etiquetas, Histograma>, servicio: &str) -> String {
    const NOMBRE: &str = "core_engine_http_request_duration_seconds";
    let mut salida = format!(
        "# HELP {NOMBRE} Duración de peticiones HTTP en segundos\n# TYPE {NOMBRE} histogram\n"
    );
    for (etiquetas, histograma) in datos {
        for (indice, limite) in BUCKETS.iter().enumerate() {
            salida.push_str(&format!(
                "{NOMBRE}_bucket{{{},le=\"{limite}\"}} {}\n",
                etiquetas_texto(etiquetas, servicio, None),
                histograma.buckets[indice]
            ));
        }
        salida.push_str(&format!(
            "{NOMBRE}_bucket{{{},le=\"+Inf\"}} {}\n",
            etiquetas_texto(etiquetas, servicio, None),
            histograma.total
        ));
        salida.push_str(&format!(
            "{NOMBRE}_sum{{{}}} {}\n",
            etiquetas_texto(etiquetas, servicio, None),
            histograma.suma
        ));
        salida.push_str(&format!(
            "{NOMBRE}_count{{{}}} {}\n",
            etiquetas_texto(etiquetas, servicio, None),
            histograma.total
        ));
    }
    salida
}

/// Reduce la URL a un patrón de cardinalidad acotada. Réplica de
/// `parametrizarRuta`: conserva segmentos estáticos y `:param`.
pub fn parametrizar_ruta(ruta: &str) -> String {
    let sin_query = ruta
        .split(['?', '#'])
        .next()
        .unwrap_or("/")
        .split('/')
        .filter(|segmento| !segmento.is_empty())
        .map(|segmento| {
            if segmento.starts_with(':') || segmento == "*" {
                segmento.to_string()
            } else if es_uuid(segmento) || es_id(segmento) {
                ":id".to_string()
            } else {
                segmento.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("/");
    format!("/{sin_query}")
}

fn es_uuid(valor: &str) -> bool {
    if valor.len() != 36 {
        return false;
    }
    let bytes = valor.as_bytes();
    bytes.iter().enumerate().all(|(indice, byte)| match indice {
        8 | 13 | 18 | 23 => *byte == b'-',
        _ => byte.is_ascii_hexdigit(),
    })
}

fn es_id(valor: &str) -> bool {
    if valor.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if valor.len() == 24 && valor.chars().all(|c| c.is_ascii_hexdigit()) {
        return true;
    }
    let mut partes = valor.split('-');
    let (Some(prefijo), Some(numero)) = (partes.next(), partes.next()) else {
        return false;
    };
    prefijo.len() >= 2
        && prefijo
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase())
        && prefijo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && numero.chars().next().is_some_and(|c| c.is_ascii_digit())
        && numero
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Timestamp ISO 8601 UTC con milisegundos y `Z` (R-C-4).
pub fn ahora_iso() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_parametrizar_uuids_y_omitir_query() {
        assert_eq!(
            parametrizar_ruta(
                "/api/v2/scheduling/recursos/123e4567-e89b-12d3-a456-426614174000?x=1"
            ),
            "/api/v2/scheduling/recursos/:id"
        );
    }

    #[test]
    fn debe_conservar_los_parametros_declarados() {
        assert_eq!(
            parametrizar_ruta("/api/v2/scheduling/citas/:citaId/cancelar"),
            "/api/v2/scheduling/citas/:citaId/cancelar"
        );
    }

    #[test]
    fn debe_parametrizar_ids_numericos_y_de_24_hex() {
        assert_eq!(parametrizar_ruta("/items/42"), "/items/:id");
        assert_eq!(
            parametrizar_ruta("/items/507f1f77bcf86cd799439011"),
            "/items/:id"
        );
    }

    #[test]
    fn debe_contar_peticiones_y_errores_relevantes() {
        let metrics = Metrics::nueva("scheduling-service");
        metrics.registrar_peticion("get", "/api/v2/scheduling/citas", 200, 50);
        metrics.registrar_peticion("get", "/api/v2/scheduling/citas", 409, 60);
        metrics.registrar_peticion("get", "/api/v2/scheduling/citas", 500, 70);
        let texto = metrics.texto();
        assert!(texto.contains(
            "core_engine_http_requests_total{estado=\"200\",metodo=\"GET\",ruta=\"/api/v2/scheduling/citas\",servicio=\"scheduling-service\"} 1"
        ));
        assert!(texto.contains("core_engine_http_relevant_client_errors_total{"));
        assert!(texto.contains("core_engine_http_server_errors_total{"));
        assert!(texto.contains("le=\"+Inf\""));
    }

    #[test]
    fn debe_exponer_contadores_y_gauges_de_negocio() {
        let metrics = Metrics::nueva("scheduling-service");
        metrics.incrementar_contador("core_engine_outbox_published_events_total");
        metrics.registrar_gauge("core_engine_outbox_pending_events", 3.0);
        let texto = metrics.texto();
        assert!(texto.contains("core_engine_outbox_published_events_total 1"));
        assert!(texto.contains("core_engine_outbox_pending_events 3"));
    }

    #[test]
    fn debe_reportar_salud_con_uptime() {
        let metrics = Metrics::nueva("scheduling-service");
        let salud = metrics.salud();
        assert_eq!(salud.servicio, "scheduling-service");
        assert!(salud.uptime_seg <= 1);
    }

    #[test]
    fn debe_marcar_estado_desconocido_fuera_de_rango() {
        let metrics = Metrics::nueva("scheduling-service");
        metrics.registrar_peticion("GET", "/x", 42, 1);
        assert!(metrics.texto().contains("estado=\"desconocido\""));
    }
}
