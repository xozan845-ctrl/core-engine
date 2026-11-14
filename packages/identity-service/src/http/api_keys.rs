//! Handlers de API keys y del intercambio por JWT corto.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::dominio::api_keys::{CrearApiKey, ROLES_API_KEY};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

const CREAR: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo("rol", true, &[Regla::En(&ROLES_API_KEY)]),
    campo("empresa_id", false, &[Regla::Uuid]),
    campo(
        "expira_en_dias",
        false,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(365.0)],
    ),
];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn actor(headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    UsuarioContexto::desde_headers_o_error(headers)
}

/// GET /api/v2/api-keys?empresa_id=...
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let claves = state
        .servicios()?
        .api_keys
        .listar(&actor, query.get("empresa_id").map(String::as_str))
        .await?;
    ok(serde_json::to_value(claves).map_err(|_| ApiError::interno())?)
}

/// POST /api/v2/api-keys
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CREAR, &dto)?;
    let actor = actor(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let creada = state
        .servicios()?
        .api_keys
        .crear(
            &actor,
            CrearApiKey {
                nombre: dto
                    .get("nombre")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                rol: dto
                    .get("rol")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                empresa_id: dto
                    .get("empresa_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                expira_en_dias: dto.get("expira_en_dias").and_then(Value::as_i64),
            },
        )
        .await?;
    created(serde_json::to_value(creada).map_err(|_| ApiError::interno())?)
}

/// DELETE /api/v2/api-keys/:id
pub async fn revocar(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let (id, estado) = state.servicios()?.api_keys.revocar(&actor, &id).await?;
    ok(serde_json::json!({ "id": id, "estado": estado }))
}

/// POST /api/v2/auth/token
pub async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let clave = headers
        .get("x-api-key")
        .and_then(|valor| valor.to_str().ok())
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .or_else(|| {
            dto.get("api_key")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|valor| !valor.is_empty())
                .map(str::to_string)
        });
    let token = state.servicios()?.api_keys.token(clave.as_deref()).await?;
    ok(serde_json::to_value(token).map_err(|_| ApiError::interno())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_validar_la_creacion_de_api_keys() {
        assert!(validar(
            CREAR,
            &json!({"nombre": "Integración", "rol": "empresario"})
        )
        .is_ok());
        assert!(validar(CREAR, &json!({"nombre": "x", "rol": "admin"})).is_err());
        assert!(validar(
            CREAR,
            &json!({"nombre": "Integración", "rol": "empresario", "expira_en_dias": 0})
        )
        .is_err());
        assert!(validar(
            CREAR,
            &json!({"nombre": "Integración", "rol": "empresario", "empresa_id": "x"})
        )
        .is_err());
    }
}
