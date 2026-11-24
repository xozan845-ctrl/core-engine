//! Endpoints internos (servicio → servicio) protegidos por clave interna.

use crate::comercio::CreditoInterno;
use crate::errors::ApiError;
use crate::http::{exigir_interna, Cuerpo};
use crate::state::AppState;
use crate::tributacion::{Declaracion, FiltrosDeclaracion, SujetoTributario};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct PurgaTenant {
    pub empresa_id: String,
    #[serde(default)]
    pub negocio_ids: Vec<String>,
    #[serde(default)]
    pub usuario_ids: Vec<String>,
}

/// POST /internal/finance/tenant/exportar
pub async fn exportar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<PurgaTenant>,
) -> Result<Json<Value>, ApiError> {
    exigir_interna(&state, &headers)?;
    let datos = state
        .facturacion
        .exportar_tenant(&dto.empresa_id, &dto.negocio_ids, &dto.usuario_ids)
        .await?;
    Ok(Json(json!({
        "servicio": "finance",
        "estado": "completado",
        "datos": datos,
    })))
}

/// POST /internal/finance/tenant/purgar
pub async fn purgar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<PurgaTenant>,
) -> Result<Json<Value>, ApiError> {
    exigir_interna(&state, &headers)?;
    let detalle = state
        .facturacion
        .purgar_tenant(&dto.empresa_id, &dto.negocio_ids, &dto.usuario_ids)
        .await?;
    Ok(Json(json!({
        "servicio": "finance",
        "estado": "completado",
        "detalle": detalle,
    })))
}

pub async fn sujeto(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(usuario_id): Path<String>,
) -> Result<Json<Option<SujetoTributario>>, ApiError> {
    exigir_interna(&state, &headers)?;
    Ok(Json(state.tributacion.sujeto_fiscal_de(&usuario_id).await?))
}

pub async fn registrar_credito(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Value>, ApiError> {
    exigir_interna(&state, &headers)?;
    let dto: CreditoInterno = serde_json::from_value(cuerpo).map_err(|_| {
        ApiError::validacion(json!([{"campo": "body", "restricciones": ["isObject"]}]))
    })?;
    if uuid::Uuid::parse_str(&dto.empresa_id).is_err()
        || uuid::Uuid::parse_str(&dto.parte_id).is_err()
    {
        return Err(ApiError::validacion(json!([
            {"campo": "empresa_id", "restricciones": ["isUuid"]},
            {"campo": "parte_id", "restricciones": ["isUuid"]}
        ])));
    }
    if uuid::Uuid::parse_str(&dto.order_id).is_err() {
        return Err(ApiError::validacion(json!([
            {"campo": "order_id", "restricciones": ["isUuid"]}
        ])));
    }
    Ok(Json(state.comercio.registrar_credito_orden(dto).await?))
}

pub async fn anular_credito(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(order_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    exigir_interna(&state, &headers)?;
    let ok = state.comercio.anular_credito_orden(&order_id).await?;
    Ok(Json(json!({"ok": ok})))
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FiltrosQuery {
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub periodo_inicio: Option<String>,
    #[serde(default)]
    pub estado: Option<String>,
}

pub async fn declaraciones(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Declaracion>>, ApiError> {
    exigir_interna(&state, &headers)?;
    Ok(Json(
        state
            .tributacion
            .declaraciones(FiltrosDeclaracion {
                empresa_id: query.get("empresa_id").cloned(),
                tipo: query.get("tipo").cloned(),
                periodo_inicio: query.get("periodo_inicio").cloned(),
                estado: query.get("estado").cloned(),
            })
            .await?,
    ))
}

/// `GET /internal/finance/configuracion-fiscal?empresa_id=&negocio_id=`.
pub async fn configuracion_fiscal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    exigir_interna(&state, &headers)?;
    let empresa = query
        .get("empresa_id")
        .map(String::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| {
            ApiError::validacion(json!([
                {"campo": "empresa_id", "restricciones": ["isString"]}
            ]))
        })?;
    if uuid::Uuid::parse_str(empresa).is_err() {
        return Err(ApiError::validacion(json!([
            {"campo": "empresa_id", "restricciones": ["isUuid"]}
        ])));
    }
    let negocio = query
        .get("negocio_id")
        .map(String::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    let config = state
        .tributacion
        .configuracion_facturacion(empresa, negocio)
        .await?;
    Ok(Json(json!({
        "empresa_id": empresa,
        "negocio_id": negocio,
        "jurisdiccion": config.jurisdiccion.codigo_pais,
        "moneda": config.jurisdiccion.moneda,
        "moneda_iso": config.jurisdiccion.moneda_iso,
        "tasa_iva": config.jurisdiccion.tasa_iva,
        "tasas": config.tasas,
        "tasa_default_codigo": config.jurisdiccion.tasa_default_codigo,
        "retencion_activa": config.retencion_activa,
        "tasa_retencion": config.retencion_activa.then_some(config.tasa_retencion),
        "retencion_codigo": config.retencion_codigo,
        "impuesto_activo": config.impuesto_activo,
        "precios_con_impuesto": config.precios_con_impuesto,
    })))
}

/// `POST /internal/finance/notas-credito` (devolución parcial desde orders).
pub async fn nota_credito(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<crate::facturacion::Comprobante>, ApiError> {
    exigir_interna(&state, &headers)?;
    let dto: crate::facturacion::NotaCreditoDto = serde_json::from_value(cuerpo).map_err(|_| {
        ApiError::validacion(json!([{"campo": "body", "restricciones": ["isObject"]}]))
    })?;
    Ok(Json(state.facturacion.emitir_nota_credito(dto).await?))
}

pub async fn tipo_cambio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    exigir_interna(&state, &headers)?;
    let base = params.get("base").map(String::as_str).unwrap_or_default();
    let quote = params.get("quote").map(String::as_str).unwrap_or_default();
    let fila = state
        .comercio
        .tipo_cambio_vigente(base, quote)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Tipo de cambio", quote))?;
    Ok(Json(fila))
}
