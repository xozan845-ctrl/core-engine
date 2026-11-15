//! Métodos de pago configurables (`/api/v2/orders/metodos-pago` y admin).

use super::{created, ok, Cuerpo};
use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::Router;

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/orders/metodos-pago", get(listar_metodos))
        .route("/api/v2/admin/metodos-pago", post(crear_metodo))
        .route(
            "/api/v2/admin/metodos-pago/{codigo}",
            patch(actualizar_metodo),
        )
}

/// `GET /api/v2/orders/metodos-pago` (cualquier usuario autenticado).
pub async fn listar_metodos(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    UsuarioContexto::desde_headers_o_error(&headers)?;
    let metodos = state.pagos()?.listar_metodos().await?;
    ok(serde_json::to_value(metodos).map_err(|_| ApiError::interno())?)
}

/// `POST /api/v2/admin/metodos-pago` (super_admin/admin).
pub async fn crear_metodo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let metodo = state.pagos()?.crear_metodo(&dto).await?;
    created(serde_json::to_value(metodo).map_err(|_| ApiError::interno())?)
}

/// `PATCH /api/v2/admin/metodos-pago/:codigo` (super_admin/admin).
pub async fn actualizar_metodo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(codigo): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let metodo = state.pagos()?.actualizar_metodo(&codigo, &dto).await?;
    ok(serde_json::to_value(metodo).map_err(|_| ApiError::interno())?)
}
