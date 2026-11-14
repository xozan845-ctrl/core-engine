//! Handlers de usuarios (`/api/v2/usuarios`).

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

const AVATAR: &[ReglaCampo] = &[campo("avatar_objeto_id", false, &[Regla::Uuid])];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

/// GET /api/v2/usuarios
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let usuarios = state
        .servicios()?
        .usuarios
        .listar(Default::default())
        .await?;
    ok(serde_json::to_value(usuarios).map_err(|_| ApiError::interno())?)
}

/// GET /api/v2/usuarios/:id
pub async fn obtener(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    if !actor.es_admin_global() && actor.user_id != id {
        return Err(ApiError::no_encontrado("Usuario", &id));
    }
    let usuario = state
        .servicios()?
        .usuarios
        .encontrar_por_id(&id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Usuario", &id))?;
    ok(serde_json::to_value(usuario).map_err(|_| ApiError::interno())?)
}

/// PATCH /api/v2/usuarios/:id/avatar
pub async fn actualizar_avatar(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(AVATAR, &dto)?;
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    let avatar = dto
        .get("avatar_objeto_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    let usuario = state
        .servicios()?
        .usuarios
        .actualizar_avatar(&id, avatar, &actor)
        .await?;
    ok(serde_json::to_value(usuario).map_err(|_| ApiError::interno())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_aceptar_un_uuid_o_null_para_el_avatar() {
        assert!(validar(AVATAR, &json!({"avatar_objeto_id": null})).is_ok());
        assert!(validar(
            AVATAR,
            &json!({"avatar_objeto_id": "11111111-1111-4111-8111-111111111111"})
        )
        .is_ok());
        assert!(validar(AVATAR, &json!({"avatar_objeto_id": "x"})).is_err());
    }
}
