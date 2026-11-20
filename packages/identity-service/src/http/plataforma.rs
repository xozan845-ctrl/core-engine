//! Suscripción de plataforma: plan de expansión y cobro manual (ADR-0046).

use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

fn actor(headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    UsuarioContexto::desde_headers_o_error(headers)
}

fn empresa_id(dto: &Value) -> Result<Option<&str>, ApiError> {
    let Some(valor) = dto.get("empresa_id") else {
        return Ok(None);
    };
    if valor.is_null() {
        return Ok(None);
    }
    let empresa = valor
        .as_str()
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser un UUID."))?;
    Uuid::parse_str(empresa)
        .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser un UUID."))?;
    Ok(Some(empresa))
}

fn referencia(dto: &Value) -> Result<&str, ApiError> {
    let valor = dto
        .get("referencia")
        .and_then(Value::as_str)
        .unwrap_or("manual")
        .trim();
    if valor.chars().count() > 120 {
        return Err(ApiError::dominio(
            "REFERENCIA_INVALIDA",
            "La referencia no puede superar 120 caracteres.",
        ));
    }
    Ok(if valor.is_empty() { "manual" } else { valor })
}

/// GET /api/v2/plataforma/planes
pub async fn planes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    actor(&headers)?;
    let planes = state.servicios()?.suscripciones.listar_planes().await?;
    Ok(Json(serde_json::json!({ "planes": planes })).into_response())
}

/// GET /api/v2/plataforma/suscripciones/me
pub async fn mi_suscripcion(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let suscripcion = state.servicios()?.suscripciones.actual(&actor).await?;
    Ok(Json(serde_json::json!({ "suscripcion": suscripcion })).into_response())
}

/// POST /api/v2/plataforma/suscripciones
pub async fn solicitar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let empresa = empresa_id(&dto)?;
    let suscripcion = state
        .servicios()?
        .suscripciones
        .crear(&actor, empresa)
        .await?;
    Ok((StatusCode::CREATED, Json(suscripcion)).into_response())
}

/// GET /api/v2/admin/plataforma/suscripciones
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let empresa = query
        .get("empresa_id")
        .map(String::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    if let Some(empresa) = empresa {
        Uuid::parse_str(empresa)
            .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser un UUID."))?;
    }
    let suscripciones = state
        .servicios()?
        .suscripciones
        .listar(&actor, empresa, query.get("estado").map(String::as_str))
        .await?;
    Ok(Json(serde_json::json!({ "suscripciones": suscripciones })).into_response())
}

/// POST /api/v2/admin/plataforma/suscripciones/:id/confirmar
pub async fn confirmar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    Uuid::parse_str(&id)
        .map_err(|_| ApiError::dominio("SUSCRIPCION_INVALIDA", "La suscripción no es un UUID."))?;
    let suscripcion = state
        .servicios()?
        .suscripciones
        .confirmar(&id, referencia(&dto)?)
        .await?;
    Ok(Json(suscripcion).into_response())
}

/// POST /api/v2/admin/plataforma/suscripciones/:id/cancelar
pub async fn cancelar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    Uuid::parse_str(&id)
        .map_err(|_| ApiError::dominio("SUSCRIPCION_INVALIDA", "La suscripción no es un UUID."))?;
    let suscripcion = state.servicios()?.suscripciones.cancelar(&id).await?;
    Ok(Json(suscripcion).into_response())
}
