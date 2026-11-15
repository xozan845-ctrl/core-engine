//! Carrito del comprador (RN-05).

use super::validacion::{campo, objeto, validar, Regla, ReglaCampo};
use super::{created, ok, Cuerpo};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::Router;
use serde_json::Value;

const AGREGAR: &[ReglaCampo] = &[
    campo("oferta_id", true, &[Regla::Texto, Regla::NoVacio]),
    campo(
        "cantidad",
        true,
        &[Regla::Entero, Regla::Min(1), Regla::Max(99)],
    ),
];

const ACTUALIZAR: &[ReglaCampo] = &[campo(
    "cantidad",
    true,
    &[Regla::Entero, Regla::Min(0), Regla::Max(99)],
)];

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/carrito", get(ver).delete(vaciar))
        .route("/api/v2/carrito/items", post(agregar))
        .route(
            "/api/v2/carrito/items/{oferta_id}",
            patch(actualizar).delete(quitar),
        )
}

async fn ver(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["comprador"], &[])?;
    ok(state.carrito()?.obtener(&usuario.user_id).await?)
}

async fn agregar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["comprador"], &[])?;
    validar(AGREGAR, &cuerpo)?;
    let datos = objeto(&cuerpo);
    let oferta_id = datos
        .get("oferta_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let cantidad = datos
        .get("cantidad")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    created(
        state
            .carrito()?
            .agregar(&usuario.user_id, oferta_id, cantidad)
            .await?,
    )
}

async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(oferta_id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["comprador"], &[])?;
    validar(ACTUALIZAR, &cuerpo)?;
    let cantidad = objeto(&cuerpo)
        .get("cantidad")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    ok(state
        .carrito()?
        .actualizar_cantidad(&usuario.user_id, &oferta_id, cantidad)
        .await?)
}

async fn quitar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(oferta_id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["comprador"], &[])?;
    ok(state
        .carrito()?
        .quitar(&usuario.user_id, &oferta_id)
        .await?)
}

async fn vaciar(State(state): State<AppState>, headers: HeaderMap) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["comprador"], &[])?;
    ok(state.carrito()?.vaciar(&usuario.user_id).await?)
}
