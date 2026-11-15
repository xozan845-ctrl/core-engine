//! Endpoints internos protegidos por `x-internal-key`.

use super::validacion::{self, campo, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::clave_interna_valida;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const PRECIOS_LOTE: &[ReglaCampo] = &[
    campo("empresa_id", true, &[Regla::Uuid]),
    campo(
        "lista_codigo",
        false,
        &[Regla::Coincide("codigo_ubicacion")],
    ),
    campo("segmento_codigo", false, &[Regla::Coincide("segmento")]),
    campo("lineas", true, &[Regla::Arreglo]),
];

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

/// `GET /internal/items/:id/cotizacion`.
pub async fn cotizacion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let empresa = query.get("empresa_id").map(String::as_str).unwrap_or("");
    let desde = query.get("desde").map(String::as_str).unwrap_or("");
    let hasta = query.get("hasta").map(String::as_str).unwrap_or("");
    ok(state
        .tarifas()?
        .cotizacion(
            empresa,
            &id,
            &Value::String(desde.to_string()),
            &Value::String(hasta.to_string()),
        )
        .await?)
}

/// `GET /internal/items/:id/precio`.
pub async fn precio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let empresa = query.get("empresa_id").map(String::as_str).unwrap_or("");
    let cantidad = query
        .get("cantidad")
        .cloned()
        .unwrap_or_else(|| "1".to_string());
    let filtros = json!({
        "lista_codigo": query.get("lista"),
        "segmento_codigo": query.get("segmento"),
    });
    let resuelto = state
        .precios()?
        .precio_de(empresa, &id, &Value::String(cantidad), &filtros)
        .await?;
    ok(json!({
        "item_id": resuelto.get("item_id"),
        "cantidad": resuelto.get("cantidad"),
        "origen": resuelto.get("origen"),
        "precio_unitario_cents": resuelto.get("precio_unitario_cents"),
        "precio_total_cents": resuelto.get("precio_total_cents"),
    }))
}

/// `POST /internal/items/precios`.
pub async fn precios_lote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    validacion::validar(PRECIOS_LOTE, &cuerpo)?;
    let lineas = cuerpo
        .get("lineas")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if lineas.is_empty() {
        return Err(ApiError::validacion(json!([{
            "campo": "lineas",
            "restricciones": ["arrayMinSize"],
        }])));
    }
    let lineas_validas = lineas.iter().all(|linea| {
        linea.as_object().is_some_and(|objeto| {
            objeto.get("clave").and_then(Value::as_str).is_some()
                && objeto
                    .get("item_id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
                && objeto
                    .get("cantidad")
                    .and_then(Value::as_i64)
                    .is_some_and(|cantidad| cantidad >= 1)
        })
    });
    if !lineas_validas {
        return Err(ApiError::validacion(json!([{
            "campo": "lineas",
            "restricciones": [],
        }])));
    }
    ok(state.precios()?.precios_lote(&cuerpo).await?)
}

/// `GET /internal/inventario/resumen`.
pub async fn resumen_inventario(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    ok(state
        .inventario()?
        .resumen(query.get("empresa_id").map(String::as_str))
        .await?)
}

/// `GET /internal/inventario/lineas`.
pub async fn lineas_inventario(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    ok(Value::Array(
        state
            .inventario()?
            .listar(query.get("empresa_id").map(String::as_str))
            .await?,
    ))
}
