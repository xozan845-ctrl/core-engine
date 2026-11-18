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

/// `POST /internal/inventario/stock` — stock y series por SKUs.
pub async fn stock_lote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(cuerpo): Json<Value>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let empresa_id = cuerpo
        .get("empresa_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let skus = cuerpo
        .get("skus")
        .and_then(Value::as_array)
        .map(|valores| {
            valores
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let ubicacion_id = cuerpo
        .get("ubicacion_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    if empresa_id.is_empty() || skus.is_empty() {
        return Err(ApiError::validacion(serde_json::json!([{
            "campo": "empresa_id/skus",
            "restricciones": [],
        }])));
    }
    ok(Value::Array(
        state
            .inventario()?
            .stock_lote(&empresa_id, &skus, ubicacion_id)
            .await?,
    ))
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

/// `POST /internal/inventario/stock/inicial`.
pub async fn stock_inicial(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(cuerpo): Json<Value>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let empresa_id = cuerpo
        .get("empresa_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let item_id = cuerpo.get("item_id").and_then(Value::as_str).unwrap_or("");
    let sku = cuerpo.get("sku").and_then(Value::as_str).unwrap_or("");
    let cantidad = cuerpo.get("cantidad").and_then(Value::as_i64).unwrap_or(-1);
    if empresa_id.is_empty() || item_id.is_empty() || cantidad < 0 {
        return Err(ApiError::validacion(serde_json::json!([{
            "campo": "empresa_id/item_id/cantidad",
            "restricciones": [],
        }])));
    }
    ok(state
        .inventario()?
        .stock_inicial(
            empresa_id,
            item_id,
            sku,
            cantidad,
            cuerpo.get("negocio_id").and_then(Value::as_str),
        )
        .await?)
}
