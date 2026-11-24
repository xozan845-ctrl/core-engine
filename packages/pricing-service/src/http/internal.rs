//! Endpoints internos de precios y cotización.

use super::validacion::{self, campo, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::clave_interna_valida;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const PRECIOS_LOTE: &[ReglaCampo] = &[
    campo("empresa_id", true, &[Regla::Uuid]),
    campo(
        "lista_codigo",
        false,
        &[Regla::Coincide("codigo_ubicacion")],
    ),
    campo("segmento_codigo", false, &[Regla::Coincide("segmento")]),
    campo("lineas", true, &[Regla::Arreglo]),
];

fn exigir_clave(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if clave_interna_valida(headers, state.config.internal_api_key.as_deref()) {
        Ok(())
    } else {
        Err(ApiError::acceso_denegado("Clave interna invalida."))
    }
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

/// `POST /internal/items/precios`.
pub async fn precios_lote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    validacion::validar(PRECIOS_LOTE, &cuerpo)?;
    let lineas = cuerpo
        .get("lineas")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if lineas.is_empty() {
        return Err(ApiError::validacion(json!([{
            "campo": "lineas",
            "restricciones": ["arrayMinSize"],
        }])));
    }
    let lineas_validas = lineas.iter().all(|linea| {
        linea.as_object().is_some_and(|objeto| {
            objeto.get("clave").and_then(Value::as_str).is_some()
                && objeto
                    .get("item_id")
                    .and_then(Value::as_str)
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
                && objeto
                    .get("cantidad")
                    .and_then(Value::as_i64)
                    .is_some_and(|cantidad| cantidad >= 1)
        })
    });
    if !lineas_validas {
        return Err(ApiError::validacion(json!([{
            "campo": "lineas",
            "restricciones": [],
        }])));
    }
    ok(state.precios()?.precios_lote(&cuerpo).await?)
}

/// `GET /internal/items/:id/precio`.
pub async fn precio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let empresa = query.get("empresa_id").map(String::as_str).unwrap_or("");
    let cantidad = query
        .get("cantidad")
        .cloned()
        .unwrap_or_else(|| "1".to_string());
    let filtros = json!({
        "lista_codigo": query.get("lista"),
        "segmento_codigo": query.get("segmento"),
    });
    let resuelto = state
        .precios()?
        .precio_de(empresa, &id, &Value::String(cantidad), &filtros)
        .await?;
    ok(json!({
        "item_id": resuelto.get("item_id"),
        "cantidad": resuelto.get("cantidad"),
        "origen": resuelto.get("origen"),
        "precio_unitario_cents": resuelto.get("precio_unitario_cents"),
        "precio_total_cents": resuelto.get("precio_total_cents"),
    }))
}

/// `GET /internal/items/:id/cotizacion`.
pub async fn cotizacion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let empresa = query.get("empresa_id").map(String::as_str).unwrap_or("");
    let desde = query.get("desde").map(String::as_str).unwrap_or("");
    let hasta = query.get("hasta").map(String::as_str).unwrap_or("");
    ok(state
        .tarifas()?
        .cotizacion(
            empresa,
            &id,
            &Value::String(desde.to_string()),
            &Value::String(hasta.to_string()),
        )
        .await?)
}

/// `PUT /internal/items/:id/precio-base`.
pub async fn precio_base(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let empresa = cuerpo
        .get("empresa_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let precio = cuerpo
        .get("precio_cents")
        .and_then(Value::as_i64)
        .ok_or_else(|| {
            ApiError::validacion(json!([{
                "campo": "precio_cents",
                "restricciones": ["required", "integer"],
            }]))
        })?;
    if empresa.is_empty() {
        return Err(ApiError::validacion(json!([{
            "campo": "empresa_id",
            "restricciones": ["required"],
        }])));
    }
    ok(state
        .precios()?
        .fijar_precio_base(empresa, &id, precio)
        .await?)
}

/// `POST /internal/cupones/validar`.
pub async fn cupon_validar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let (empresa, codigo, monto) = datos_cupon(&cuerpo)?;
    let validado = state.cupones()?.validar(&empresa, &codigo, monto).await?;
    ok(json!({
        "cupon_id": validado.cupon_id,
        "codigo": validado.codigo,
        "descuento_cents": validado.descuento_cents,
    }))
}

/// `POST /internal/cupones/canjear`.
pub async fn cupon_canjear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let (empresa, codigo, monto) = datos_cupon(&cuerpo)?;
    let order_id = cuerpo
        .get("order_id")
        .and_then(Value::as_str)
        .filter(|valor| uuid::Uuid::parse_str(valor).is_ok())
        .ok_or_else(|| {
            ApiError::validacion(json!([{
                "campo": "order_id",
                "restricciones": ["isUuid"],
            }]))
        })?;
    let validado = state
        .cupones()?
        .canjear(&empresa, &codigo, order_id, monto)
        .await?;
    ok(json!({
        "cupon_id": validado.cupon_id,
        "codigo": validado.codigo,
        "descuento_cents": validado.descuento_cents,
    }))
}

/// `POST /internal/cupones/liberar`.
pub async fn cupon_liberar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let (empresa, codigo, _) = datos_cupon(&cuerpo)?;
    let order_id = cuerpo
        .get("order_id")
        .and_then(Value::as_str)
        .filter(|valor| uuid::Uuid::parse_str(valor).is_ok())
        .ok_or_else(|| {
            ApiError::validacion(json!([{
                "campo": "order_id",
                "restricciones": ["isUuid"],
            }]))
        })?;
    state
        .cupones()?
        .liberar(&empresa, &codigo, order_id)
        .await?;
    ok(json!({ "ok": true }))
}

fn datos_cupon(cuerpo: &Value) -> Result<(String, String, i64), ApiError> {
    let empresa = cuerpo
        .get("empresa_id")
        .and_then(Value::as_str)
        .filter(|valor| uuid::Uuid::parse_str(valor).is_ok())
        .ok_or_else(|| {
            ApiError::validacion(json!([{
                "campo": "empresa_id",
                "restricciones": ["isUuid"],
            }]))
        })?
        .to_string();
    let codigo = cuerpo
        .get("codigo")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| {
            ApiError::validacion(json!([{
                "campo": "codigo",
                "restricciones": ["isString"],
            }]))
        })?
        .to_string();
    let monto = cuerpo
        .get("monto_cents")
        .and_then(Value::as_i64)
        .filter(|monto| *monto >= 0)
        .ok_or_else(|| {
            ApiError::validacion(json!([{
                "campo": "monto_cents",
                "restricciones": ["isInt", "min:0"],
            }]))
        })?;
    Ok((empresa, codigo, monto))
}

/// POST /internal/pricing/tenant/exportar
pub async fn exportar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<crate::lifecycle::TenantRequest>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let datos = crate::lifecycle::exportar(state.db()?.pool(), &dto)
        .await
        .map_err(error_interno)?;
    ok(json!({ "servicio": "pricing", "estado": "completado", "datos": datos }))
}

/// POST /internal/pricing/tenant/purgar
pub async fn purgar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<crate::lifecycle::TenantRequest>,
) -> Result<Response, ApiError> {
    exigir_clave(&state, &headers)?;
    let detalle = crate::lifecycle::purgar(state.db()?.pool(), &dto)
        .await
        .map_err(error_interno)?;
    ok(json!({ "servicio": "pricing", "estado": "completado", "detalle": detalle }))
}

fn error_interno(_: sqlx::Error) -> ApiError {
    ApiError::interno()
}
