//! Endpoints internos de inventario protegidos por `x-internal-key`.

use crate::auth::clave_interna_valida;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Query, State};
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

/// `GET /internal/inventario/resumen`.
pub async fn resumen(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    ok(state
        .inventario()?
        .resumen(
            query.get("empresa_id").map(String::as_str),
            query.get("negocio_id").map(String::as_str),
        )
        .await?)
}

/// `GET /internal/inventario/lineas`.
pub async fn lineas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    ok(Value::Array(
        state
            .inventario()?
            .listar(
                query.get("empresa_id").map(String::as_str),
                query.get("negocio_id").map(String::as_str),
            )
            .await?,
    ))
}
