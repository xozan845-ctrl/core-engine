//! Métodos de pago configurables (`/api/v2/orders/metodos-pago` y admin).

use super::{created, ok, Cuerpo};
use crate::auth::UsuarioContexto;
use crate::checkout::PAYMENT_PROCESADO;
use crate::errors::ApiError;
use crate::pagos::Pago;
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
        .route("/internal/pagos/webhook", post(webhook_pago))
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
    confirmar_y_avanzar(&state, pago, &actor.user_id).await
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
    let motivo = motivo_de(&dto);
    let pago = state
        .pagos()?
        .rechazar(&id, &motivo, &actor.user_id)
        .await?;
    rechazar_y_cancelar(&state, pago, &motivo, &actor.user_id).await
}

/// `POST /internal/pagos/webhook` (proveedor de pagos; secreto compartido).
pub async fn webhook_pago(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let esperado = state.config.pago_webhook_secret.as_deref().unwrap_or("");
    let recibido = headers
        .get("x-webhook-secret")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("");
    if esperado.is_empty() || recibido != esperado {
        return Err(ApiError::acceso_denegado("Secreto de webhook invalido."));
    }
    let pago_id = dto
        .get("pago_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| {
            ApiError::validacion(json!([
                {"campo": "pago_id", "restricciones": ["isString"]}
            ]))
        })?;
    let estado = dto
        .get("estado")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let actor = dto
        .get("actor")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .unwrap_or("00000000-0000-4000-8000-000000000000");
    match estado {
        "confirmado" => {
            let pago = state.pagos()?.confirmar(pago_id, actor).await?;
            confirmar_y_avanzar(&state, pago, actor).await
        }
        "rechazado" => {
            let motivo = motivo_de(&dto);
            let pago = state.pagos()?.rechazar(pago_id, &motivo, actor).await?;
            rechazar_y_cancelar(&state, pago, &motivo, actor).await
        }
        _ => Err(ApiError::validacion(json!([
            {"campo": "estado", "restricciones": ["isIn"]}
        ]))),
    }
}

fn motivo_de(dto: &Value) -> String {
    dto.get("motivo")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .unwrap_or("pago_rechazado")
        .to_string()
}

async fn confirmar_y_avanzar(
    state: &AppState,
    pago: Pago,
    actor_id: &str,
) -> Result<Response, ApiError> {
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
            Some(actor_id),
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

async fn rechazar_y_cancelar(
    state: &AppState,
    pago: Pago,
    motivo: &str,
    actor_id: &str,
) -> Result<Response, ApiError> {
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
            Some(motivo),
            Some(actor_id),
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
