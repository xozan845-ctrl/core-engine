//! Handlers de la región tributaria del tenant (ADR-0049).
//!
//! - `GET /api/v2/regiones-tributarias` alimenta el selector del onboarding.
//! - `GET /api/v2/empresas/{id}/region-tributaria` muestra la elección actual.
//! - `PUT /api/v2/empresas/{id}/region-tributaria` la cambia con historial.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const CAMBIAR: &[ReglaCampo] = &[
    campo(
        "region_tributaria",
        true,
        &[Regla::Texto, Regla::MaxLongitud(16)],
    ),
    campo("motivo", false, &[Regla::Texto, Regla::MaxLongitud(300)]),
];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn serializar<T: serde::Serialize>(valor: T) -> Result<Value, ApiError> {
    serde_json::to_value(valor).map_err(|_| ApiError::interno())
}

/// GET /api/v2/regiones-tributarias?pack_codigo=&pack_version=
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario", "vendedor"], &[])?;
    let pack = query
        .get("pack_codigo")
        .map(|valor| valor.trim().to_lowercase())
        .filter(|valor| !valor.is_empty());
    let version = match query.get("pack_version") {
        Some(valor) if !valor.trim().is_empty() => {
            Some(valor.trim().parse::<i32>().map_err(|_| {
                ApiError::validacion(json!([
                    {"campo": "pack_version", "restricciones": ["isInt", "min:1"]}
                ]))
            })?)
        }
        _ => None,
    };
    if pack.is_none() && version.is_some() {
        return Err(ApiError::validacion(json!([
            {"campo": "pack_codigo", "restricciones": ["required"]}
        ])));
    }
    let regiones = state
        .servicios()?
        .regiones
        .listar(pack.as_deref(), version)
        .await?;
    ok(serializar(regiones)?)
}

/// GET /api/v2/empresas/{id}/region-tributaria
pub async fn obtener(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario", "vendedor"], &[])?;
    let region = state
        .servicios()?
        .regiones
        .de_empresa(&empresa_id)
        .await?
        .ok_or_else(|| {
            ApiError::dominio(
                "REGION_NO_ASIGNADA",
                "La empresa no tiene región tributaria asignada.",
            )
        })?;
    ok(serializar(region)?)
}

/// PUT /api/v2/empresas/{id}/region-tributaria
pub async fn actualizar(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CAMBIAR, &dto)?;
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let region = dto
        .get("region_tributaria")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let motivo = dto.get("motivo").and_then(Value::as_str).unwrap_or("");
    let region = state
        .servicios()?
        .regiones
        .asignar(&empresa_id, region, motivo, &actor)
        .await?;
    ok(serializar(region)?)
}
