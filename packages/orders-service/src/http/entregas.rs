//! Entregas digitales (Fase P2.1).

use super::ok;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::get;
use axum::Router;

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/pedidos/{id}/entregas-digitales", get(listar))
        .route("/api/v2/entregas-digitales/{token}", get(descargar))
}

async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["super_admin", "admin", "comprador"], &[])?;
    super::exigir_capacidades(&state, &usuario, &["fulfillment_digital"]).await?;
    ok(state.entregas()?.listar_por_orden(&id, &usuario).await?)
}

async fn descargar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(token): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["super_admin", "admin", "comprador"], &[])?;
    super::exigir_capacidades(&state, &usuario, &["fulfillment_digital"]).await?;
    ok(state.entregas()?.descargar(&token, &usuario).await?)
}
