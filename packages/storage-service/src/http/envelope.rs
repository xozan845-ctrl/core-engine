//! Envelope de éxito `{data, meta}` (R-C-1), igual que el resto de servicios.

use crate::metrics::ahora_iso;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::{json, Value};

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
    let valor: Value = match serde_json::from_slice(&bytes) {
        Ok(valor) => valor,
        Err(_) => return Response::from_parts(parts, Body::from(bytes)),
    };
    if valor.get("meta").is_some() && valor.get("data").is_some() {
        return Response::from_parts(parts, Body::from(bytes));
    }
    let nuevo = serde_json::to_vec(&envolver(
        valor,
        &path,
        method.as_str(),
        parts.status.as_u16(),
    ))
    .unwrap_or_else(|_| bytes.to_vec());
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json; charset=utf-8"),
    );
    Response::from_parts(parts, Body::from(nuevo))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_excluir_infraestructura_y_media_publica() {
        assert!(!ruta_lleva_envelope("/api/v2/storage/public/objetos/x"));
        assert!(!ruta_lleva_envelope("/internal/storage/tenants/x"));
        assert!(ruta_lleva_envelope("/api/v2/storage/objetos"));
    }
}
