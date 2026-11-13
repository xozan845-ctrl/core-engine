//! Handlers de inventario (`/api/v2/admin/inventario`).

use super::validacion::{self, campo, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::{UsuarioContexto, ROLES_INVENTARIO_ADMIN};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

const CAP_STOCK: &[&str] = &["inventario_stock"];
const CAP_LOTES: &[&str] = &["inventario_lotes"];

const CREAR_PROVEEDOR: &[ReglaCampo] = &[
    campo("empresa_id", true, &[Regla::Uuid]),
    campo("codigo", true, &[Regla::Texto]),
    campo("nombre", true, &[Regla::Texto]),
    campo("correo", false, &[Regla::Texto]),
    campo("telefono", false, &[Regla::Texto]),
];

const CREAR_LOTE: &[ReglaCampo] = &[
    campo("empresa_id", true, &[Regla::Uuid]),
    campo("item_id", true, &[Regla::Uuid]),
    campo("proveedor_id", false, &[Regla::Uuid]),
    campo("codigo_lote", true, &[Regla::Texto]),
    campo("vence_en", false, &[Regla::Fecha]),
    campo("cantidad_inicial", true, &[Regla::Entero, Regla::Min(0.0)]),
];

const CONSUMIR_LOTES: &[ReglaCampo] = &[
    campo("empresa_id", true, &[Regla::Uuid]),
    campo("item_id", true, &[Regla::Uuid]),
    campo("cantidad", true, &[Regla::Entero, Regla::Min(1.0)]),
    campo("codigo_lote", false, &[Regla::Texto]),
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
        return usuario
            .tenant_id
            .clone()
            .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."));
    }
    solicitada
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indica empresa_id."))
}

/// `GET /api/v2/admin/inventario`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_INVENTARIO_ADMIN, &[], CAP_STOCK).await?;
    let empresa = if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        Some(
            usuario
                .tenant_id
                .as_deref()
                .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."))?,
        )
    } else {
        query.get("empresa_id").map(String::as_str)
    };
    ok(Value::Array(state.inventario()?.listar(empresa).await?))
}

/// `POST /api/v2/admin/inventario/proveedores`.
pub async fn crear_proveedor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    autorizar(
        &state,
        &headers,
        ROLES_INVENTARIO_ADMIN,
        &[],
        &[CAP_LOTES[0], CAP_STOCK[0]],
    )
    .await?;
    validar(CREAR_PROVEEDOR, &cuerpo)?;
    created(state.inventario()?.crear_proveedor(&cuerpo).await?)
}

/// `POST /api/v2/admin/inventario/lotes`.
pub async fn crear_lote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    autorizar(
        &state,
        &headers,
        ROLES_INVENTARIO_ADMIN,
        &[],
        &[CAP_LOTES[0], CAP_STOCK[0]],
    )
    .await?;
    validar(CREAR_LOTE, &cuerpo)?;
    created(state.inventario()?.crear_lote(&cuerpo).await?)
}

/// `POST /api/v2/admin/inventario/lotes/consumir`.
pub async fn consumir_lote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    autorizar(
        &state,
        &headers,
        ROLES_INVENTARIO_ADMIN,
        &[],
        &[CAP_LOTES[0], CAP_STOCK[0]],
    )
    .await?;
    validar(CONSUMIR_LOTES, &cuerpo)?;
    created(state.inventario()?.consumir_lote(&cuerpo).await?)
}

/// `GET /api/v2/admin/inventario/lotes`.
pub async fn listar_lotes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &state,
        &headers,
        ROLES_INVENTARIO_ADMIN,
        &[],
        &[CAP_LOTES[0], CAP_STOCK[0]],
    )
    .await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(Value::Array(
        state
            .inventario()?
            .listar_lotes(&empresa, query.get("item_id").map(String::as_str))
            .await?,
    ))
}
