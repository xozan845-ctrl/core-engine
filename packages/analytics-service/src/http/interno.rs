//! Endpoints internos protegidos por clave (servicio → servicio).

use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;
use std::collections::HashMap;

pub fn rutas() -> Router<AppState> {
    Router::new().route(
        "/internal/analytics/reportes/ejecutar",
        post(ejecutar_reportes),
    )
}

async fn ejecutar_reportes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    exigir_interna(&state, &headers)?;
    let limite = params
        .get("limite")
        .and_then(|valor| valor.parse::<i64>().ok())
        .unwrap_or(20)
        .clamp(1, 100);
    let ejecutados = crate::reportes::ejecutar_vencidos(state.db()?, limite).await?;
    Ok(Json(json!({ "ejecutados": ejecutados })))
}

fn exigir_interna(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let esperada = state.config.internal_api_key.as_deref().unwrap_or_default();
    let recibida = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or_default();
    if esperada.is_empty() || recibida != esperada {
        return Err(ApiError::acceso_denegado(
            "La ruta interna exige la clave de servicio.",
        ));
    }
    Ok(())
}
