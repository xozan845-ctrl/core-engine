//! Handlers de composición (recetas/kits).

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
    campo("componente_item_id", true, &[Regla::Uuid]),
    campo("cantidad", true, &[Regla::Entero, Regla::Min(1.0)]),
];
const ACTUALIZAR: &[ReglaCampo] = &[campo("cantidad", true, &[Regla::Entero, Regla::Min(1.0)])];

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

/// `GET /api/v2/catalog/items/:id/componentes`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state.componentes()?.listar(&empresa, &item_id).await?)
}

/// `POST /api/v2/catalog/items/:id/componentes`.
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
    created(
        state
            .componentes()?
            .crear(&empresa, &item_id, &cuerpo)
            .await?,
    )
}

/// `PATCH /api/v2/catalog/items/:id/componentes/:componenteId`.
pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((item_id, componente_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(ACTUALIZAR, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .componentes()?
        .actualizar(&empresa, &item_id, &componente_id, &cuerpo)
        .await?)
}

/// `DELETE /api/v2/catalog/items/:id/componentes/:componenteId`.
pub async fn eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((item_id, componente_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .componentes()?
        .eliminar(&empresa, &item_id, &componente_id)
        .await?)
}
