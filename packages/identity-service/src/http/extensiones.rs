//! Handlers de extensiones configurables por empresa.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;

const CREAR: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo(
        "recurso",
        true,
        &[Regla::Texto, Regla::Coincide("segmento")],
    ),
    campo(
        "descripcion",
        false,
        &[Regla::Texto, Regla::MaxLongitud(1000)],
    ),
    campo("esquema", true, &[Regla::Objeto]),
    campo("formulario", false, &[Regla::Objeto]),
    campo("reglas", false, &[Regla::Objeto]),
    campo("activo", false, &[Regla::Booleano]),
];

const ESTADO: &[ReglaCampo] = &[campo("activo", true, &[Regla::Booleano])];
const VALIDAR: &[ReglaCampo] = &[campo("datos", true, &[Regla::Objeto])];
const VALORES: &[ReglaCampo] = &[campo("valores", true, &[Regla::Objeto])];

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn actor(headers: &HeaderMap, escritura: bool) -> Result<UsuarioContexto, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(headers)?;
    let roles: &[&str] = if escritura {
        &["super_admin", "admin", "empresario"]
    } else {
        &[
            "super_admin",
            "admin",
            "empresario",
            "vendedor",
            "coordinador",
            "supervisor",
            "operativo",
        ]
    };
    actor.exigir_roles(roles, &[])?;
    Ok(actor)
}

/// GET /api/v2/empresas/:empresaId/extensiones
pub async fn listar(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers, false)?;
    let extensiones = state
        .servicios()?
        .extensiones
        .listar(&empresa_id, &actor)
        .await?;
    ok(Value::Array(extensiones))
}

/// POST /api/v2/empresas/:empresaId/extensiones
pub async fn crear(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CREAR, &dto)?;
    let actor = actor(&headers, true)?;
    let extension = state
        .servicios()?
        .extensiones
        .crear(&empresa_id, &actor, &dto)
        .await?;
    created(extension)
}

/// PATCH /api/v2/empresas/:empresaId/extensiones/:codigo
pub async fn cambiar_estado(
    State(state): State<AppState>,
    Path((empresa_id, codigo)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(ESTADO, &dto)?;
    let actor = actor(&headers, true)?;
    let extension = state
        .servicios()?
        .extensiones
        .cambiar_estado(
            &empresa_id,
            &codigo,
            dto.get("activo").and_then(Value::as_bool).unwrap_or(true),
            &actor,
        )
        .await?;
    ok(extension)
}

/// POST /api/v2/empresas/:empresaId/extensiones/:codigo/validar
pub async fn validar_datos(
    State(state): State<AppState>,
    Path((empresa_id, codigo)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(VALIDAR, &dto)?;
    let actor = actor(&headers, false)?;
    let resultado = state
        .servicios()?
        .extensiones
        .validar(
            &empresa_id,
            &codigo,
            dto.get("datos").unwrap_or(&Value::Null),
            &actor,
        )
        .await?;
    ok(resultado)
}

/// GET /api/v2/empresas/:empresaId/extensiones/:codigo/valores/:recursoId
pub async fn obtener_valores(
    State(state): State<AppState>,
    Path((empresa_id, codigo, recurso_id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers, false)?;
    let valores = state
        .servicios()?
        .extensiones
        .obtener_valores(&empresa_id, &codigo, &recurso_id, &actor)
        .await?;
    ok(valores.unwrap_or(Value::Null))
}

/// PUT /api/v2/empresas/:empresaId/extensiones/:codigo/valores/:recursoId
pub async fn guardar_valores(
    State(state): State<AppState>,
    Path((empresa_id, codigo, recurso_id)): Path<(String, String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(VALORES, &dto)?;
    let actor = actor(&headers, true)?;
    let valores = state
        .servicios()?
        .extensiones
        .guardar_valores(
            &empresa_id,
            &codigo,
            &recurso_id,
            dto.get("valores").unwrap_or(&Value::Null),
            &actor,
        )
        .await?;
    ok(valores)
}

/// DELETE /api/v2/empresas/:empresaId/extensiones/:codigo/valores/:recursoId
pub async fn eliminar_valores(
    State(state): State<AppState>,
    Path((empresa_id, codigo, recurso_id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers, true)?;
    let respuesta = state
        .servicios()?
        .extensiones
        .eliminar_valores(&empresa_id, &codigo, &recurso_id, &actor)
        .await?;
    ok(respuesta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_validar_los_dtos_de_extensiones() {
        assert!(validar(
            CREAR,
            &json!({
                "codigo": "ficha_cliente",
                "nombre": "Ficha",
                "recurso": "cliente",
                "esquema": {"type": "object"}
            })
        )
        .is_ok());
        assert!(validar(CREAR, &json!({"codigo": "ficha"})).is_err());
        assert!(validar(VALIDAR, &json!({"datos": {}})).is_ok());
        assert!(validar(VALIDAR, &json!({"datos": []})).is_err());
        assert!(validar(VALORES, &json!({"valores": {}})).is_ok());
    }
}
