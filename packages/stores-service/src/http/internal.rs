//! Endpoints internos (servicio → servicio) protegidos con clave interna.

use super::ofertas::ids_de_lote;
use super::validacion::{self, ReglaCampo};
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

fn exigir_clave(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if clave_interna_valida(headers, state.config.internal_api_key.as_deref()) {
        Ok(())
    } else {
        Err(ApiError::acceso_denegado("Clave interna invalida."))
    }
}

fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    validacion::validar(esquema, cuerpo)
}

fn ids(query: &HashMap<String, String>) -> Vec<String> {
    ids_de_lote(query.get("ids").map(String::as_str).unwrap_or(""))
}

/// `GET /internal/ofertas`.
pub async fn ofertas_por_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    Ok(Json(Value::Array(state.ofertas()?.por_ids(&ids(&query)).await?)).into_response())
}

/// `GET /internal/ofertas/producto/:productoId/activas`.
pub async fn ofertas_activas_producto(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(producto_id): Path<String>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let total = state
        .ofertas()?
        .contar_activas_por_producto(&producto_id)
        .await?;
    Ok(Json(json!({ "total": total })).into_response())
}

/// `GET /internal/tiendas`.
pub async fn tiendas_por_ids(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let tiendas = state.tiendas()?.por_ids(&ids(&query)).await?;
    Ok(Json(json!({ "tiendas": tiendas })).into_response())
}

/// `GET /internal/tiendas/empresa/:empresaId`.
pub async fn tiendas_de_empresa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(empresa_id): Path<String>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let tiendas = state.tiendas()?.de_empresa(&empresa_id).await?;
    Ok(Json(json!({ "tiendas": tiendas })).into_response())
}

/// `GET /internal/tiendas/vendedor/:vendedorId`.
pub async fn tienda_de_vendedor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(vendedor_id): Path<String>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let tienda = state.tiendas()?.de_vendedor(&vendedor_id).await?;
    Ok(Json(json!({ "tienda": tienda })).into_response())
}

/// `GET /internal/tiendas/:id`.
pub async fn tienda_por_id(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let tienda = state
        .tiendas()?
        .encontrar_por_id(&id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Tienda", &id))?;
    Ok(Json(tienda).into_response())
}

/// `POST /internal/tiendas/:id/asignar-vendedor`.
pub async fn asignar_vendedor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    validar(validacion::ASIGNAR_VENDEDOR, &cuerpo)?;
    let tienda = state
        .tiendas()?
        .encontrar_por_id(&id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Tienda", &id))?;
    state
        .tiendas()?
        .asignar_vendedor(
            &id,
            tienda
                .get("empresa_id")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            cuerpo
                .get("usuario_id")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            cuerpo
                .get("participacion")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
        )
        .await?;
    Ok((axum::http::StatusCode::CREATED, Json(json!({ "ok": true }))).into_response())
}

/// `POST /internal/tiendas/:id/desasignar-vendedor`.
pub async fn desasignar_vendedor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    validar(validacion::DESASIGNAR_VENDEDOR, &cuerpo)?;
    state
        .tiendas()?
        .desasignar_vendedor(
            &id,
            cuerpo
                .get("usuario_id")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .await?;
    Ok((axum::http::StatusCode::CREATED, Json(json!({ "ok": true }))).into_response())
}
