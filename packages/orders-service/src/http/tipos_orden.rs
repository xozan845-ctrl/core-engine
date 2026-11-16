//! Administración de tipos de orden y transiciones (Fase 3).

use super::{created, ok, Cuerpo};
use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::Router;

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/admin/tipos-orden", get(listar).post(crear))
        .route(
            "/api/v2/admin/tipos-orden/{codigo}",
            get(detalle).patch(actualizar),
        )
        .route(
            "/api/v2/admin/tipos-orden/{codigo}/transiciones",
            post(agregar_transicion),
        )
        .route(
            "/api/v2/admin/tipos-orden/{codigo}/transiciones/{desde}/{hasta}",
            delete(eliminar_transicion),
        )
}

fn admin(headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    Ok(actor)
}

pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    admin(&headers)?;
    let tipos = state.tipos()?.listar().await?;
    ok(serde_json::to_value(tipos).map_err(|_| ApiError::interno())?)
}

pub async fn detalle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(codigo): Path<String>,
) -> Result<Response, ApiError> {
    admin(&headers)?;
    ok(state.tipos()?.detalle(&codigo).await?)
}

pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    admin(&headers)?;
    let tipo = state.tipos()?.crear(&dto).await?;
    created(serde_json::to_value(tipo).map_err(|_| ApiError::interno())?)
}

pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(codigo): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    admin(&headers)?;
    let tipo = state.tipos()?.actualizar(&codigo, &dto).await?;
    ok(serde_json::to_value(tipo).map_err(|_| ApiError::interno())?)
}

pub async fn agregar_transicion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(codigo): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    admin(&headers)?;
    let desde = dto
        .get("desde")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let hasta = dto
        .get("hasta")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let transicion = state
        .tipos()?
        .agregar_transicion(&codigo, desde, hasta)
        .await?;
    created(serde_json::to_value(transicion).map_err(|_| ApiError::interno())?)
}

pub async fn eliminar_transicion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((codigo, desde, hasta)): Path<(String, String, String)>,
) -> Result<Response, ApiError> {
    admin(&headers)?;
    state
        .tipos()?
        .eliminar_transicion(&codigo, &desde, &hasta)
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
