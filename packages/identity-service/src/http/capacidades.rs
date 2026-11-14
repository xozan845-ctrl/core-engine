//! Handlers de capacidades por empresa.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;

const ACTUALIZAR: &[ReglaCampo] = &[
    campo("activo", true, &[Regla::Booleano]),
    campo("config", false, &[Regla::Objeto]),
];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

/// GET /api/v2/empresas/:empresaId/capacidades
pub async fn listar(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let capacidades = state
        .servicios()?
        .capacidades
        .listar(&empresa_id, &actor)
        .await?;
    ok(serde_json::to_value(capacidades).map_err(|_| ApiError::interno())?)
}

/// PATCH /api/v2/empresas/:empresaId/capacidades/:capacidad
pub async fn actualizar(
    State(state): State<AppState>,
    Path((empresa_id, capacidad)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(ACTUALIZAR, &dto)?;
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let activo = dto.get("activo").and_then(Value::as_bool).ok_or_else(|| {
        ApiError::validacion(serde_json::json!([
            {"campo": "activo", "restricciones": ["isBoolean"]}
        ]))
    })?;
    let config = dto.get("config").filter(|valor| !valor.is_null()).cloned();
    let capacidad = state
        .servicios()?
        .capacidades
        .actualizar(&empresa_id, &capacidad, activo, config, &actor)
        .await?;
    ok(serde_json::to_value(capacidad).map_err(|_| ApiError::interno())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_exigir_activo_y_aceptar_config_opcional() {
        assert!(validar(ACTUALIZAR, &json!({})).is_err());
        assert!(validar(ACTUALIZAR, &json!({"activo": true})).is_ok());
        assert!(validar(ACTUALIZAR, &json!({"activo": "si"})).is_err());
        assert!(validar(ACTUALIZAR, &json!({"activo": true, "config": {}})).is_ok());
        assert!(validar(ACTUALIZAR, &json!({"activo": true, "config": []})).is_err());
    }
}
