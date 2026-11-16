//! Catálogo de fulfillments (`/api/v2/orders/fulfillments` y admin).

use super::{created, ok, Cuerpo};
use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/orders/fulfillments", get(listar))
        .route("/api/v2/admin/fulfillments", post(crear))
        .route(
            "/api/v2/admin/fulfillments/{codigo}",
            axum::routing::patch(actualizar),
        )
}

pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    UsuarioContexto::desde_headers_o_error(&headers)?;
    let fulfillments = state.fulfillments()?.listar_activos().await?;
    ok(serde_json::to_value(fulfillments).map_err(|_| ApiError::interno())?)
}

pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let fulfillment = state.fulfillments()?.crear(&dto).await?;
    created(serde_json::to_value(fulfillment).map_err(|_| ApiError::interno())?)
}

pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(codigo): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let fulfillment = state.fulfillments()?.actualizar(&codigo, &dto).await?;
    ok(serde_json::to_value(fulfillment).map_err(|_| ApiError::interno())?)
}
