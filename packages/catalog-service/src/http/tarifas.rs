//! Handlers de tarifas por fecha y cotización.

use super::empresa::de_tenant;
use super::validacion::{self, campo, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::ROLES_CATALOGO;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

const CAPACIDAD: &[&str] = &["catalogo_items"];
const FECHA: &str = "^\\d{4}-\\d{2}-\\d{2}$";

const CREAR: &[ReglaCampo] = &[
    campo("desde", true, &[Regla::Coincide("fecha_corta")]),
    campo("hasta", true, &[Regla::Coincide("fecha_corta")]),
    campo("precio_cents", true, &[Regla::Entero, Regla::Min(0.0)]),
];
const ACTUALIZAR: &[ReglaCampo] = &[
    campo("desde", false, &[Regla::Coincide("fecha_corta")]),
    campo("hasta", false, &[Regla::Coincide("fecha_corta")]),
    campo("precio_cents", false, &[Regla::Entero, Regla::Min(0.0)]),
];

fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    let _ = FECHA;
    validacion::validar(esquema, cuerpo)
}
fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}
fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}
fn empresa(
    usuario: &crate::auth::UsuarioContexto,
    query: &HashMap<String, String>,
) -> Result<String, ApiError> {
    de_tenant(
        usuario,
        query.get("empresa_id").map(String::as_str),
        "No puedes operar otra empresa.",
    )
}

/// `GET /api/v2/catalog/items/:id/tarifas`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state.tarifas()?.listar(&empresa, &item_id).await?)
}

/// `POST /api/v2/catalog/items/:id/tarifas`.
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(CREAR, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    created(state.tarifas()?.crear(&empresa, &item_id, &cuerpo).await?)
}

/// `PATCH /api/v2/catalog/items/:id/tarifas/:tarifaId`.
pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((item_id, tarifa_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(ACTUALIZAR, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .tarifas()?
        .actualizar(&empresa, &item_id, &tarifa_id, &cuerpo)
        .await?)
}

/// `DELETE /api/v2/catalog/items/:id/tarifas/:tarifaId`.
pub async fn eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((item_id, tarifa_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .tarifas()?
        .eliminar(&empresa, &item_id, &tarifa_id)
        .await?)
}

/// `GET /api/v2/catalog/items/:id/cotizacion`.
pub async fn cotizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    let desde = query.get("desde").map(String::as_str).unwrap_or("");
    let hasta = query.get("hasta").map(String::as_str).unwrap_or("");
    ok(state
        .tarifas()?
        .cotizacion(
            &empresa,
            &item_id,
            &Value::String(desde.to_string()),
            &Value::String(hasta.to_string()),
        )
        .await?)
}
