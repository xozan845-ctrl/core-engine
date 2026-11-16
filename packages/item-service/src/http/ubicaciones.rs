//! Handlers de ubicaciones y stock por ubicación.

use super::validacion::{self, campo, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::{UsuarioContexto, ROLES_CATALOGO};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

const CAPACIDAD: &[&str] = &["inventario_stock"];
const TIPOS: &[&str] = &["bodega", "sucursal", "almacen", "otra"];

const CREAR: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Coincide("codigo_ubicacion")]),
    campo("nombre", true, &[Regla::Texto]),
    campo("tipo", false, &[Regla::En(TIPOS)]),
    campo("direccion", false, &[Regla::Texto]),
    campo("predeterminada", false, &[Regla::Booleano]),
];

const ACTUALIZAR: &[ReglaCampo] = &[
    campo("codigo", false, &[Regla::Coincide("codigo_ubicacion")]),
    campo("nombre", false, &[Regla::Texto]),
    campo("tipo", false, &[Regla::En(TIPOS)]),
    campo("direccion", false, &[Regla::Texto]),
    campo("predeterminada", false, &[Regla::Booleano]),
    campo("activo", false, &[Regla::Booleano]),
];

const AJUSTAR: &[ReglaCampo] = &[campo("stock", true, &[Regla::Entero, Regla::Min(0.0)])];

const TRASLADAR: &[ReglaCampo] = &[
    campo("item_id", true, &[Regla::Uuid]),
    campo("origen_id", true, &[Regla::Uuid]),
    campo("destino_id", true, &[Regla::Uuid]),
    campo("cantidad", true, &[Regla::Entero, Regla::Min(1.0)]),
];

fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    validacion::validar(esquema, cuerpo)
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn empresa(usuario: &UsuarioContexto, solicitada: Option<&str>) -> Result<String, ApiError> {
    if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        let tenant = usuario
            .tenant_id
            .as_deref()
            .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."))?;
        if let Some(solicitada) = solicitada {
            if !solicitada.is_empty() && solicitada != tenant {
                return Err(ApiError::dominio(
                    "EMPRESA_NO_AUTORIZADA",
                    "No puedes operar otra empresa.",
                ));
            }
        }
        return Ok(tenant.to_string());
    }
    solicitada
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indica empresa_id."))
}

/// `GET /api/v2/admin/ubicaciones`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(Value::Array(state.ubicaciones()?.listar(&empresa).await?))
}

/// `POST /api/v2/admin/ubicaciones`.
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(CREAR, &cuerpo)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    created(state.ubicaciones()?.crear(&empresa, &cuerpo).await?)
}

/// `PATCH /api/v2/admin/ubicaciones/:id`.
pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(ACTUALIZAR, &cuerpo)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .ubicaciones()?
        .actualizar(&empresa, &id, &cuerpo)
        .await?)
}

/// `POST /api/v2/admin/ubicaciones/transferencias`.
pub async fn transferir(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(TRASLADAR, &cuerpo)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    created(state.ubicaciones()?.transferir(&empresa, &cuerpo).await?)
}

/// `GET /api/v2/admin/ubicaciones/items/:itemId/stock`.
pub async fn stock_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .ubicaciones()?
        .stock_por_item(&empresa, &item_id)
        .await?)
}

/// `GET /api/v2/admin/ubicaciones/:id/stock`.
pub async fn stock_ubicacion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(Value::Array(
        state
            .ubicaciones()?
            .stock_de_ubicacion(&empresa, &id)
            .await?,
    ))
}

/// `PUT /api/v2/admin/ubicaciones/:id/stock/:itemId`.
pub async fn fijar_stock(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, item_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(AJUSTAR, &cuerpo)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .ubicaciones()?
        .fijar_stock(&empresa, &id, &item_id, &cuerpo)
        .await?)
}
