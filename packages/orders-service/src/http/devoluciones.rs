//! Handlers RMA: devoluciones parciales por línea.

use super::{autorizar, created, ok, Cuerpo};
use crate::devoluciones::CrearDevolucionInput;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/orders/{id}/devoluciones", get(listar).post(crear))
        .route("/api/v2/admin/devoluciones/{id}/aprobar", post(aprobar))
        .route("/api/v2/admin/devoluciones/{id}/recibir", post(recibir))
        .route("/api/v2/admin/devoluciones/{id}/rechazar", post(rechazar))
        .route(
            "/api/v2/admin/devoluciones/{id}/reembolsar",
            post(reembolsar),
        )
        .route(
            "/api/v2/admin/devoluciones/{id}/nota-credito",
            post(nota_credito),
        )
}

async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &headers,
        &["super_admin", "admin", "empresario", "comprador"],
        &[],
    )?;
    let input: CrearDevolucionInput =
        serde_json::from_value(dto).map_err(|_| ApiError::interno())?;
    let devolucion = state
        .devoluciones()?
        .crear(&order_id, &usuario, input)
        .await?;
    created(serde_json::to_value(devolucion).map_err(|_| ApiError::interno())?)
}

async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &headers,
        &["super_admin", "admin", "empresario", "comprador"],
        &[],
    )?;
    let devoluciones = state
        .devoluciones()?
        .listar_por_orden(&order_id, &usuario)
        .await?;
    ok(serde_json::to_value(devoluciones).map_err(|_| ApiError::interno())?)
}

async fn aprobar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&headers, &["super_admin", "admin"], &[])?;
    let devolucion = state.devoluciones()?.aprobar(&id, &usuario).await?;
    ok(serde_json::to_value(devolucion).map_err(|_| ApiError::interno())?)
}

async fn recibir(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&headers, &["super_admin", "admin"], &[])?;
    let devolucion = state.devoluciones()?.recibir(&id, &usuario).await?;
    ok(serde_json::to_value(devolucion).map_err(|_| ApiError::interno())?)
}

async fn rechazar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&headers, &["super_admin", "admin"], &[])?;
    let devolucion = state.devoluciones()?.rechazar(&id, &usuario).await?;
    ok(serde_json::to_value(devolucion).map_err(|_| ApiError::interno())?)
}

async fn reembolsar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&headers, &["super_admin", "admin"], &[])?;
    let devolucion = state
        .devoluciones()?
        .reembolsar_devolucion(&id, &usuario)
        .await?;
    ok(serde_json::to_value(devolucion).map_err(|_| ApiError::interno())?)
}

async fn nota_credito(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&headers, &["super_admin", "admin"], &[])?;
    let devolucion = state
        .devoluciones()?
        .emitir_nota_credito(&id, &usuario)
        .await?;
    ok(serde_json::to_value(devolucion).map_err(|_| ApiError::interno())?)
}
