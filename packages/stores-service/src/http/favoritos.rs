//! Wishlist del comprador.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::Value;

const FAVORITO: &[ReglaCampo] = &[campo("item_id", true, &[Regla::Uuid])];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/comprador/favoritos", get(listar).post(agregar))
        .route(
            "/api/v2/comprador/favoritos/{item_id}",
            axum::routing::delete(eliminar),
        )
}

async fn listar(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &["comprador"], &[], &[]).await?;
    ok(
        serde_json::to_value(state.favoritos()?.listar(&usuario.user_id).await?)
            .map_err(|_| ApiError::interno())?,
    )
}

async fn agregar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &["comprador"], &[], &[]).await?;
    validar(FAVORITO, &dto)?;
    let item_id = dto.get("item_id").and_then(Value::as_str).unwrap_or("");
    state
        .favoritos()?
        .agregar(&usuario.user_id, item_id)
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &["comprador"], &[], &[]).await?;
    state
        .favoritos()?
        .eliminar(&usuario.user_id, &item_id)
        .await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
