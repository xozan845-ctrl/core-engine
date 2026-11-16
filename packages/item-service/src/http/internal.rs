//! Endpoints internos protegidos por `x-internal-key`.

use crate::auth::clave_interna_valida;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

fn exigir_clave(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if clave_interna_valida(headers, state.config.internal_api_key.as_deref()) {
        Ok(())
    } else {
        Err(ApiError::acceso_denegado("Clave interna invalida."))
    }
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn ids_de_lote(valor: &str) -> Vec<String> {
    valor
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}

/// `GET /internal/items/lote`.
pub async fn lote_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let skus = ids_de_lote(query.get("skus").map(String::as_str).unwrap_or(""));
    ok(Value::Array(state.items()?.lote_por_skus(&skus).await?))
}

/// `GET /internal/items/:id`.
pub async fn item_por_id(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let item = state
        .items()?
        .encontrar_por_id(&id, None)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Item", &id))?;
    ok(item)
}
