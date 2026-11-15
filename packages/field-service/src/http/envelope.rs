//! Envelope de éxito `{data, meta}` (R-C-1).
//!
//! Réplica de `response-envelope.interceptor.ts`: envuelve solo las respuestas
//! públicas JSON 2xx y deja intactas las rutas de infraestructura, los errores
//! y las respuestas sin cuerpo.

use crate::metrics::ahora_iso;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{json, Value};

/// Rutas que no usan el envelope: infraestructura, docs y binarios públicos.
const RUTAS_SIN_ENVELOPE: [&str; 5] = [
    "/internal",
    "/health",
    "/metrics",
    "/docs",
    "/api/v2/storage/public",
];

pub fn ruta_lleva_envelope(ruta: &str) -> bool {
    !RUTAS_SIN_ENVELOPE
        .iter()
        .any(|prefijo| ruta.starts_with(prefijo))
}

/// `true` si el cuerpo ya es `{data, meta:{timestamp,path,method,statusCode}}`.
pub fn es_respuesta_api(valor: &Value) -> bool {
    let Some(objeto) = valor.as_object() else {
        return false;
    };
    let Some(meta) = objeto.get("meta").and_then(Value::as_object) else {
        return false;
    };
    objeto.contains_key("data")
        && meta.contains_key("timestamp")
        && meta.contains_key("path")
        && meta.contains_key("method")
        && meta.contains_key("statusCode")
}

pub fn envolver(data: Value, path: &str, method: &str, status: u16) -> Value {
    json!({
        "data": data,
        "meta": {
            "timestamp": ahora_iso(),
            "path": path,
            "method": method,
            "statusCode": status,
        },
    })
}

/// Aplica el envelope a un cuerpo JSON si corresponde. Devuelve `None` cuando
/// la respuesta debe pasar sin cambios (no 2xx, 204, no JSON o ya envuelta).
pub fn aplicar_envelope(
    path: &str,
    method: &str,
    status: StatusCode,
    cuerpo: &[u8],
) -> Option<Vec<u8>> {
    if !ruta_lleva_envelope(path)
        || status == StatusCode::NO_CONTENT
        || !status.is_success()
        || cuerpo.is_empty()
    {
        return None;
    }
    let valor: Value = serde_json::from_slice(cuerpo).ok()?;
    if es_respuesta_api(&valor) {
        return None;
    }
    serde_json::to_vec(&envolver(valor, path, method, status.as_u16())).ok()
}

/// Middleware equivalente al interceptor global.
pub async fn envelope(request: Request, next: Next) -> Response {
    let path = request
        .uri()
        .path_and_query()
        .map(|valor| valor.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let method = request.method().clone();
    let response = next.run(request).await;

    if !ruta_lleva_envelope(&path)
        || response.status() == StatusCode::NO_CONTENT
        || !response.status().is_success()
    {
        return response;
    }

    let (mut parts, body) = response.into_parts();
    let Ok(bytes) = body
        .collect()
        .await
        .map(|recolectado| recolectado.to_bytes())
    else {
        return Response::from_parts(parts, Body::empty());
    };
    match aplicar_envelope(&path, method.as_str(), parts.status, &bytes) {
        Some(nuevo) => {
            parts.headers.remove(header::CONTENT_LENGTH);
            parts.headers.insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/json; charset=utf-8"),
            );
            Response::from_parts(parts, Body::from(nuevo))
        }
        None => Response::from_parts(parts, Body::from(bytes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_excluir_las_rutas_de_infraestructura() {
        assert!(!ruta_lleva_envelope("/internal/partes"));
        assert!(!ruta_lleva_envelope("/health"));
        assert!(!ruta_lleva_envelope("/metrics"));
        assert!(!ruta_lleva_envelope("/docs"));
        assert!(ruta_lleva_envelope("/api/v2/field/rutas"));
    }

    #[test]
    fn debe_envolver_un_cuerpo_json_nuevo() {
        let cuerpo = aplicar_envelope(
            "/api/v2/field/rutas",
            "GET",
            StatusCode::OK,
            br#"{"items":[]}"#,
        )
        .expect("envuelto");
        let valor: Value = serde_json::from_slice(&cuerpo).expect("json");
        assert_eq!(valor["data"]["items"], json!([]));
        assert_eq!(valor["meta"]["path"], "/api/v2/field/rutas");
        assert_eq!(valor["meta"]["method"], "GET");
        assert_eq!(valor["meta"]["statusCode"], 200);
        assert!(valor["meta"]["timestamp"]
            .as_str()
            .expect("timestamp")
            .ends_with('Z'));
    }

    #[test]
    fn debe_respetar_un_cuerpo_ya_envuelto() {
        let ya = json!({
            "data": [],
            "meta": {"timestamp": "2026-11-11T00:00:00.000Z", "path": "/x", "method": "GET", "statusCode": 200}
        });
        assert!(aplicar_envelope(
            "/api/v2/field/rutas",
            "GET",
            StatusCode::OK,
            ya.to_string().as_bytes()
        )
        .is_none());
    }

    #[test]
    fn debe_dejar_pasar_errores_y_respuestas_sin_cuerpo() {
        assert!(aplicar_envelope(
            "/api/v2/field/rutas",
            "GET",
            StatusCode::BAD_REQUEST,
            br#"{"codigo":"VALIDACION"}"#
        )
        .is_none());
        assert!(
            aplicar_envelope("/api/v2/field/rutas", "DELETE", StatusCode::NO_CONTENT, b"")
                .is_none()
        );
    }
}
