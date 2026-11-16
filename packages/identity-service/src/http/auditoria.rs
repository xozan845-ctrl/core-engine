//! Consulta de auditoría por empresa y negocio.

use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn fecha(valor: Option<&String>) -> Result<Option<String>, ApiError> {
    let Some(valor) = valor.filter(|valor| !valor.is_empty()) else {
        return Ok(None);
    };
    let valido = chrono::NaiveDate::parse_from_str(valor, "%Y-%m-%d").is_ok()
        || chrono::DateTime::parse_from_rfc3339(valor).is_ok();
    if !valido {
        return Err(ApiError::validacion(json!([
            {"campo": "fecha", "restricciones": ["isDate"]}
        ])));
    }
    Ok(Some(valor.to_string()))
}

/// GET /api/v2/empresas/:empresaId/auditoria
pub async fn listar(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &["auditoria.leer"])?;
    let negocio = actor
        .negocio_id
        .clone()
        .or_else(|| query.get("negocio_id").cloned().filter(|v| !v.is_empty()));
    let desde = fecha(query.get("desde"))?;
    let hasta = fecha(query.get("hasta"))?;
    let filas = state
        .servicios()?
        .auditoria
        .listar(
            &actor,
            &empresa_id,
            negocio.as_deref(),
            query.get("accion").map(String::as_str),
            desde.as_deref(),
            hasta.as_deref(),
        )
        .await?;
    ok(Value::Array(filas))
}
