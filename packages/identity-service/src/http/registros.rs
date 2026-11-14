//! Handlers de registros genéricos por parte.

use super::validacion::{campo, validar, Regla, ReglaCampo};
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

const CREAR: &[ReglaCampo] = &[
    campo("tipo", true, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("titulo", false, &[Regla::Texto, Regla::MaxLongitud(200)]),
    campo("datos", false, &[Regla::Objeto]),
    campo("adjuntos", false, &[Regla::Arreglo]),
    campo("cita_id", false, &[Regla::Uuid]),
    campo("item_id", false, &[Regla::Uuid]),
];

const ACTUALIZAR: &[ReglaCampo] = &[
    campo("tipo", false, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("titulo", false, &[Regla::Texto, Regla::MaxLongitud(200)]),
    campo("datos", false, &[Regla::Objeto]),
    campo("adjuntos", false, &[Regla::Arreglo]),
    campo("cita_id", false, &[Regla::Uuid]),
    campo("item_id", false, &[Regla::Uuid]),
];

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn actor(headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(headers)?;
    actor.exigir_roles(
        &[
            "super_admin",
            "admin",
            "empresario",
            "vendedor",
            "coordinador",
            "supervisor",
            "operativo",
        ],
        &[],
    )?;
    Ok(actor)
}

pub async fn listar(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let registros = state
        .servicios()?
        .registros
        .listar(
            &empresa_id,
            &parte_id,
            &actor,
            query.get("tipo").map(String::as_str),
            query
                .get("limite")
                .and_then(|valor| valor.parse::<i64>().ok()),
        )
        .await?;
    ok(Value::Array(registros))
}

pub async fn crear(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CREAR, &dto)?;
    let actor = actor(&headers)?;
    let registro = state
        .servicios()?
        .registros
        .crear(&empresa_id, &parte_id, &actor, &dto)
        .await?;
    created(registro)
}

pub async fn obtener(
    State(state): State<AppState>,
    Path((empresa_id, parte_id, registro_id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let registro = state
        .servicios()?
        .registros
        .obtener(&empresa_id, &parte_id, &registro_id, &actor)
        .await?;
    ok(registro)
}

pub async fn actualizar(
    State(state): State<AppState>,
    Path((empresa_id, parte_id, registro_id)): Path<(String, String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(ACTUALIZAR, &dto)?;
    let actor = actor(&headers)?;
    let registro = state
        .servicios()?
        .registros
        .actualizar(&empresa_id, &parte_id, &registro_id, &actor, &dto)
        .await?;
    ok(registro)
}

pub async fn eliminar(
    State(state): State<AppState>,
    Path((empresa_id, parte_id, registro_id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let respuesta = state
        .servicios()?
        .registros
        .eliminar(&empresa_id, &parte_id, &registro_id, &actor)
        .await?;
    ok(respuesta)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_validar_el_dto_de_registro() {
        assert!(validar(
            CREAR,
            &json!({
                "tipo": "historia_clinica",
                "titulo": "Consulta",
                "datos": {"nota": "ok"},
                "adjuntos": ["11111111-1111-4111-8111-111111111111"]
            })
        )
        .is_ok());
        assert!(validar(CREAR, &json!({"tipo": "1malo"})).is_err());
        assert!(validar(CREAR, &json!({"tipo": "nota", "datos": []})).is_err());
        assert!(validar(
            ACTUALIZAR,
            &json!({"cita_id": "11111111-1111-4111-8111-111111111111"})
        )
        .is_ok());
    }
}
