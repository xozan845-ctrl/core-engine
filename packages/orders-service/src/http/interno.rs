//! Endpoints internos (servicio → servicio) protegidos por clave interna.

use super::{created, ok, query};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use serde_json::json;
use std::collections::HashMap;

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/internal/orders", get(listar))
        .route("/internal/orders/{id}", get(orden))
        .route("/internal/orders/{id}/transicion", post(transicion))
        .route("/internal/orders/{id}/reproyectar", post(reproyectar))
        .route("/internal/orders/{id}/historia", get(historia))
}

async fn transicion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    let Some(estado) = query(&params, "estado") else {
        return Err(ApiError::no_encontrado("Estado", "requerido"));
    };
    let vista = state
        .checkout()?
        .transicionar(
            &id,
            estado,
            query(&params, "motivo"),
            query(&params, "actor").or(Some("logistica")),
            None,
        )
        .await?;
    created(vista)
}

async fn orden(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    let orden = state
        .vistas()?
        .encontrar(state.db()?, &id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Orden", &id))?;
    ok(orden)
}

async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    ok(state
        .vistas()?
        .listar_todo(state.db()?, query(&params, "estado"), None)
        .await?)
}

async fn reproyectar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    created(state.checkout()?.reproyectar(&id).await?)
}

async fn historia(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    let historia = crate::event_store::OrderEventStore
        .historia_de(state.db()?, &id)
        .await?;
    if historia.is_empty() {
        return Err(ApiError::no_encontrado("Orden", &id));
    }
    let salida: Vec<serde_json::Value> = historia
        .into_iter()
        .map(|evento| {
            json!({
                "event_id": evento.event_id,
                "aggregate_id": evento.aggregate_id,
                "tipo": evento.tipo,
                "payload": evento.payload,
                "version": evento.version,
                "creado_en": evento.creado_en,
            })
        })
        .collect();
    ok(salida)
}
