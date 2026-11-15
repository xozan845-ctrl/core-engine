//! Handlers de unidades serializadas.

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

const CREAR: &[ReglaCampo] = &[
    campo("serial", true, &[Regla::Coincide("serial")]),
    campo("imei", false, &[Regla::Coincide("imei")]),
    campo("ubicacion_id", false, &[Regla::Uuid]),
    campo("metadata", false, &[Regla::Objeto]),
];
const ACTUALIZAR: &[ReglaCampo] = &[
    campo("serial", false, &[Regla::Coincide("serial")]),
    campo("imei", false, &[Regla::Coincide("imei")]),
    campo("ubicacion_id", false, &[Regla::Uuid]),
    campo("metadata", false, &[Regla::Objeto]),
    campo("estado", false, &[Regla::En(&["disponible", "baja"])]),
];
const HABILITAR: &[ReglaCampo] = &[campo("activo", true, &[Regla::Booleano])];

fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
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

/// `GET /api/v2/catalog/items/:id/series`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state.series()?.listar(&empresa, &item_id).await?)
}

/// `POST /api/v2/catalog/items/:id/series`.
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
    created(state.series()?.crear(&empresa, &item_id, &cuerpo).await?)
}

/// `PATCH /api/v2/catalog/items/:id/serializacion`.
pub async fn habilitar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(HABILITAR, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .series()?
        .habilitar_serializacion(
            &empresa,
            &item_id,
            cuerpo
                .get("activo")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .await?)
}

/// `PATCH /api/v2/catalog/series/:serieId`.
pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(ACTUALIZAR, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    ok(state.series()?.actualizar(&empresa, &id, &cuerpo).await?)
}

/// `GET /api/v2/catalog/series/:serial`.
pub async fn buscar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .series()?
        .buscar_por_serial(&empresa, &Value::String(id))
        .await?)
}
