//! Handlers de negocios por empresa (ADR-0019).

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::dominio::negocios::CrearNegocio;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const NEGOCIO: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo(
        "vertical_id",
        true,
        &[Regla::Texto, Regla::Coincide("segmento")],
    ),
    campo(
        "pack_codigo",
        true,
        &[Regla::Texto, Regla::Coincide("segmento")],
    ),
    campo("pack_version", false, &[Regla::Entero, Regla::Min(1.0)]),
];

const PLANTILLA_QUERY: &[ReglaCampo] = &[campo(
    "vertical_id",
    true,
    &[Regla::Texto, Regla::Coincide("segmento")],
)];

const ESTADO: &[ReglaCampo] = &[campo(
    "estado",
    true,
    &[Regla::En(&["activo", "suspendido", "archivado"])],
)];

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn actor(headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    Ok(actor)
}

/// GET /api/v2/empresas/{empresa_id}/negocios
pub async fn listar(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let negocios = state
        .servicios()?
        .negocios
        .listar(&actor, &empresa_id)
        .await?;
    ok(Value::Array(negocios))
}

/// POST /api/v2/empresas/{empresa_id}/negocios
pub async fn crear(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(NEGOCIO, &dto)?;
    let actor = actor(&headers)?;
    let negocio = state
        .servicios()?
        .negocios
        .crear(
            &actor,
            &empresa_id,
            CrearNegocio {
                codigo: texto(&dto, "codigo"),
                nombre: texto(&dto, "nombre"),
                vertical_id: texto(&dto, "vertical_id"),
                pack_codigo: texto(&dto, "pack_codigo"),
                pack_version: dto
                    .get("pack_version")
                    .and_then(Value::as_i64)
                    .and_then(|valor| i32::try_from(valor).ok()),
            },
        )
        .await?;
    created(negocio)
}

/// GET /api/v2/negocios/plantillas?vertical_id=retail
pub async fn plantillas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    actor(&headers)?;
    let vertical = query
        .get("vertical_id")
        .map(|valor| valor.trim().to_lowercase())
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| ApiError::dominio("VERTICAL_REQUERIDA", "Indica vertical_id."))?;
    let datos = json!({ "vertical_id": vertical });
    validar(PLANTILLA_QUERY, &datos)?;
    let filas = state.servicios()?.negocios.plantillas(&vertical).await?;
    ok(Value::Array(filas))
}

/// POST /api/v2/empresas/{empresa_id}/negocios/{negocio_id}/plantillas
pub async fn aplicar_plantilla(
    State(state): State<AppState>,
    Path((empresa_id, negocio_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let creados = state
        .servicios()?
        .negocios
        .aplicar_plantilla(&actor, &empresa_id, &negocio_id)
        .await?;
    created(json!({ "roles_creados": creados }))
}

/// PATCH /api/v2/empresas/{empresa_id}/negocios/{negocio_id}/estado
pub async fn archivar(
    State(state): State<AppState>,
    Path((empresa_id, negocio_id)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(ESTADO, &dto)?;
    let actor = actor(&headers)?;
    let negocio = state
        .servicios()?
        .negocios
        .archivar(
            &actor,
            &empresa_id,
            &negocio_id,
            dto.get("estado")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .await?;
    ok(negocio)
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
