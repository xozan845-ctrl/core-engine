//! Endpoints internos de inventario protegidos por `x-internal-key`.

use crate::auth::clave_interna_valida;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
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

/// `POST /internal/inventario/devolucion` — restock parcial idempotente.
pub async fn devolucion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(cuerpo): Json<Value>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let order_id = cuerpo
        .get("order_id")
        .and_then(Value::as_str)
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| {
            ApiError::validacion(serde_json::json!([{
                "campo": "order_id",
                "restricciones": ["isUuid"],
            }]))
        })?;
    let devolucion_id = cuerpo
        .get("devolucion_id")
        .and_then(Value::as_str)
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| {
            ApiError::validacion(serde_json::json!([{
                "campo": "devolucion_id",
                "restricciones": ["isUuid"],
            }]))
        })?;
    let items = cuerpo
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        return Err(ApiError::validacion(serde_json::json!([{
            "campo": "items",
            "restricciones": ["arrayMinSize"],
        }])));
    }
    let mut tx = state
        .db
        .as_ref()
        .ok_or_else(ApiError::interno)?
        .transaccion()
        .await
        .map_err(crate::db::interno)?;
    crate::reservas::Reservas::reintegrar_lineas_en_transaccion(
        &mut tx,
        order_id,
        devolucion_id,
        &Value::Array(items),
    )
    .await?;
    tx.commit().await.map_err(crate::db::interno)?;
    ok(serde_json::json!({ "ok": true }))
}

/// POST /internal/inventory/tenant/exportar
pub async fn exportar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<crate::lifecycle::TenantRequest>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let datos = crate::lifecycle::exportar(state.db()?.pool(), &dto)
        .await
        .map_err(error_interno)?;
    ok(json!({ "servicio": "inventory", "estado": "completado", "datos": datos }))
}

/// POST /internal/inventory/tenant/purgar
pub async fn purgar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<crate::lifecycle::TenantRequest>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let detalle = crate::lifecycle::purgar(state.db()?.pool(), &dto)
        .await
        .map_err(error_interno)?;
    ok(json!({ "servicio": "inventory", "estado": "completado", "detalle": detalle }))
}

fn error_interno(_: sqlx::Error) -> ApiError {
    ApiError::interno()
}
