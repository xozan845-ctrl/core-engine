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
                tipo: query.get("tipo").cloned(),
                periodo_inicio: query.get("periodo_inicio").cloned(),
                estado: query.get("estado").cloned(),
            })
            .await?,
    ))
}
