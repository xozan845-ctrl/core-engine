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
        .route(
            "/api/v2/admin/proveedores-pago",
            get(listar_proveedores).post(crear_proveedor),
        )
        .route(
            "/api/v2/admin/proveedores-pago/{codigo}",
            patch(actualizar_proveedor),
        )
        .route(
            "/api/v2/empresas/{id}/configuracion-pagos",
            get(consultar_configuracion).put(guardar_configuracion),
        )
        .route("/api/v2/admin/pagos", get(listar_pagos))
        .route("/api/v2/admin/pagos/{id}/confirmar", post(confirmar_pago))
        .route("/api/v2/admin/pagos/{id}/rechazar", post(rechazar_pago))
        .route("/api/v2/admin/pagos/{id}/reembolsar", post(reembolsar_pago))
        .route(
            "/api/v2/admin/pagos/{id}/contracargo",
            post(contracargo_pago),
        )
        .route("/api/v2/admin/pagos/{id}/conciliar", post(conciliar_pago))
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

/// `POST /api/v2/admin/pagos/:id/reembolsar` (admin).
pub async fn reembolsar_pago(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let monto_cents = dto
        .get("monto_cents")
        .and_then(Value::as_i64)
        .filter(|monto| *monto > 0)
        .ok_or_else(|| {
            ApiError::validacion(json!([
                {"campo": "monto_cents", "restricciones": ["isInt", "min:1"]}
            ]))
        })?;
    let motivo = dto
        .get("motivo")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let pago = state
        .pagos()?
        .reembolsar(&id, monto_cents, motivo, &actor.user_id)
        .await?;
    ok(serde_json::to_value(pago).map_err(|_| ApiError::interno())?)
}

/// `POST /api/v2/admin/pagos/:id/contracargo` (admin).
pub async fn contracargo_pago(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let monto_cents = dto
        .get("monto_cents")
        .and_then(Value::as_i64)
        .filter(|monto| *monto > 0)
        .ok_or_else(|| {
            ApiError::validacion(json!([
                {"campo": "monto_cents", "restricciones": ["isInt", "min:1"]}
            ]))
        })?;
    let motivo = dto
        .get("motivo")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let pago = state
        .pagos()?
        .contracargar(&id, monto_cents, motivo, &actor.user_id)
        .await?;
    ok(serde_json::to_value(pago).map_err(|_| ApiError::interno())?)
}

/// `POST /api/v2/admin/pagos/:id/conciliar` (admin).
pub async fn conciliar_pago(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let pago = state.pagos()?.conciliar(&id, &actor.user_id).await?;
    ok(serde_json::to_value(pago).map_err(|_| ApiError::interno())?)
}

/// `GET /api/v2/admin/proveedores-pago`.
pub async fn listar_proveedores(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let proveedores = state.pagos()?.listar_proveedores().await?;
    ok(serde_json::to_value(proveedores).map_err(|_| ApiError::interno())?)
}

/// `POST /api/v2/admin/proveedores-pago`.
pub async fn crear_proveedor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let proveedor = state.pagos()?.crear_proveedor(&dto).await?;
    created(serde_json::to_value(proveedor).map_err(|_| ApiError::interno())?)
}

/// `PATCH /api/v2/admin/proveedores-pago/:codigo`.
pub async fn actualizar_proveedor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(codigo): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let proveedor = state.pagos()?.actualizar_proveedor(&codigo, &dto).await?;
    ok(serde_json::to_value(proveedor).map_err(|_| ApiError::interno())?)
}

/// `GET /api/v2/empresas/:id/configuracion-pagos`.
pub async fn consultar_configuracion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    autorizar_empresa(&actor, &id)?;
    let negocio_id = negocio_de(&query)?;
    let config = state
        .pagos()?
        .configuracion(&id, negocio_id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Configuracion de pagos", &id))?;
    ok(serde_json::to_value(config).map_err(|_| ApiError::interno())?)
}

/// `PUT /api/v2/empresas/:id/configuracion-pagos`.
pub async fn guardar_configuracion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    autorizar_empresa(&actor, &id)?;
    let negocio_id = negocio_de(&query)?;
    let config = state
        .pagos()?
        .guardar_configuracion(&id, negocio_id, &dto)
        .await?;
    ok(serde_json::to_value(config).map_err(|_| ApiError::interno())?)
}

fn autorizar_empresa(actor: &UsuarioContexto, empresa_id: &str) -> Result<(), ApiError> {
    if actor.es_admin_global() {
        return Ok(());
    }
    if actor.rol == "empresario" && actor.tenant_id.as_deref() == Some(empresa_id) {
        return Ok(());
    }
    Err(ApiError::acceso_denegado(
        "No tienes acceso a la configuracion de pagos de esta empresa.",
    ))
}

fn negocio_de(query: &HashMap<String, String>) -> Result<Option<&str>, ApiError> {
    let negocio = query
        .get("negocio_id")
        .map(String::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    if let Some(negocio) = negocio {
        if uuid::Uuid::parse_str(negocio).is_err() {
            return Err(ApiError::validacion(json!([
                {"campo": "negocio_id", "restricciones": ["isUuid"]}
            ])));
        }
    }
    Ok(negocio)
}

/// `POST /internal/pagos/webhook` (proveedor de pagos; secreto compartido).
pub async fn webhook_pago(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
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
    let esperado = state
        .pagos()?
        .secreto_webhook(pago_id)
        .await?
        .or_else(|| state.config.pago_webhook_secret.clone())
        .unwrap_or_default();
    let recibido = headers
        .get("x-webhook-secret")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("");
    if esperado.is_empty() || recibido != esperado {
        return Err(ApiError::acceso_denegado("Secreto de webhook invalido."));
    }
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
    // `transicionar` emite `order.cancelled` para liberar stock. Un rechazo de
    // pago ocurre antes de que exista una venta, así que no corresponde emitir
    // `devolucion.solicitada` ni revertir asientos contables.
    state
        .checkout()?
        .transicionar(
            &pago.order_id,
            "cancelada",
            Some(motivo),
            Some(actor_id),
            None,
        )
        .await?;
    ok(serde_json::to_value(pago).map_err(|_| ApiError::interno())?)
}
