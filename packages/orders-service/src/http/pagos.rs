//! Métodos de pago configurables (`/api/v2/orders/metodos-pago` y admin).

use super::{created, ok, Cuerpo};
use crate::auth::UsuarioContexto;
use crate::checkout::PAYMENT_PROCESADO;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::Router;
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/orders/metodos-pago", get(listar_metodos))
        .route("/api/v2/admin/metodos-pago", post(crear_metodo))
        .route(
            "/api/v2/admin/metodos-pago/{codigo}",
            patch(actualizar_metodo),
        )
        .route("/api/v2/admin/pagos", get(listar_pagos))
        .route("/api/v2/admin/pagos/{id}/confirmar", post(confirmar_pago))
        .route("/api/v2/admin/pagos/{id}/rechazar", post(rechazar_pago))
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

/// `GET /api/v2/admin/pagos?order_id=...` (admin).
pub async fn listar_pagos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let order_id = query
        .get("order_id")
        .map(String::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| {
            ApiError::validacion(json!([
                {"campo": "order_id", "restricciones": ["isString"]}
            ]))
        })?;
    let pagos = state.pagos()?.listar_por_orden(order_id).await?;
    ok(serde_json::to_value(pagos).map_err(|_| ApiError::interno())?)
}

/// `POST /api/v2/admin/pagos/:id/confirmar` (admin).
pub async fn confirmar_pago(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let pago = state.pagos()?.confirmar(&id, &actor.user_id).await?;
    let vista = state
        .vistas()?
        .encontrar(state.db()?, &pago.order_id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Orden", &pago.order_id))?;
    state
        .checkout()?
        .transicionar(
            &pago.order_id,
            "pagada",
            Some("pago_confirmado"),
            Some(&actor.user_id),
            Some((
                PAYMENT_PROCESADO,
                json!({
                    "order_id": pago.order_id,
                    "monto_cents": pago.monto_cents,
                    "moneda": pago.moneda,
                    "estado": "procesado",
                    "metodo": pago.metodo_codigo,
                    "items": vista.items.iter().map(|item| json!({
                        "sku": item.sku,
                        "cantidad": item.cantidad,
                        "empresa_id": item.empresa_id,
                    })).collect::<Vec<_>>(),
                }),
            )),
        )
        .await?;
    ok(serde_json::to_value(pago).map_err(|_| ApiError::interno())?)
}

/// `POST /api/v2/admin/pagos/:id/rechazar` (admin).
pub async fn rechazar_pago(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let motivo = dto
        .get("motivo")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .unwrap_or("pago_rechazado")
        .to_string();
    let pago = state
        .pagos()?
        .rechazar(&id, &motivo, &actor.user_id)
        .await?;
    let vista = state
        .vistas()?
        .encontrar(state.db()?, &pago.order_id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Orden", &pago.order_id))?;
    let items: Vec<Value> = vista
        .items
        .iter()
        .map(|item| json!({"sku": item.sku, "cantidad": item.cantidad}))
        .collect();
    state
        .checkout()?
        .transicionar(
            &pago.order_id,
            "cancelada",
            Some(&motivo),
            Some(&actor.user_id),
            Some((
                "devolucion.solicitada",
                json!({
                    "order_id": pago.order_id,
                    "items": items,
                    "motivo": motivo,
                }),
            )),
        )
        .await?;
    ok(serde_json::to_value(pago).map_err(|_| ApiError::interno())?)
}
