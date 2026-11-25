//! Rutas públicas de commissions-service.
//!
//! Reproduce los controladores NestJS: rol del endpoint + capacidad
//! `liquidaciones`, envelope global y códigos `200`/`201`.

use crate::auth::{exigir_capacidad, UsuarioContexto};
use crate::db::hoy_local;
use crate::errors::ApiError;
use crate::liquidaciones::periodo_a_cerrar;
use crate::liquidaciones::{Liquidacion, LiquidacionEmpresa, Periodo};
use crate::reportes::ReporteKpi;
use crate::state::AppState;
use crate::ventas::{EstadoCuenta, EstadoCuentaEmpresa};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use chrono::NaiveDate;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

const CAPACIDAD_LIQUIDACIONES: &str = "liquidaciones";
const ROLES_VENDEDOR: [&str; 1] = ["vendedor"];
const ROLES_EMPRESARIO: [&str; 1] = ["empresario"];
const ROLES_ADMIN: [&str; 2] = ["super_admin", "admin"];

#[derive(Debug, Deserialize)]
pub struct CorteQuery {
    pub inicio: Option<String>,
    pub fin: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PagarQuery {
    pub estado: Option<String>,
}

/// Mismo orden que los guards globales: Rol primero, capacidad después.
async fn autorizar(
    state: &AppState,
    headers: &HeaderMap,
    roles: &[&str],
) -> Result<UsuarioContexto, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(headers)?;
    usuario.exigir_roles(roles, &[])?;
    exigir_capacidad(
        state.capacidades.as_ref(),
        &usuario,
        &[CAPACIDAD_LIQUIDACIONES],
    )
    .await?;
    Ok(usuario)
}

pub async fn mis_liquidaciones(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Liquidacion>>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_VENDEDOR).await?;
    Ok(Json(
        state.liquidaciones.de_vendedor(&usuario.user_id).await?,
    ))
}

pub async fn liquidaciones_empresa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<LiquidacionEmpresa>>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESARIO).await?;
    let Some(tenant) = usuario.tenant_id.as_deref() else {
        return Err(ApiError::no_encontrado("Empresa", "activa"));
    };
    let negocio = negocio_de(&usuario, &params);
    Ok(Json(
        state
            .liquidaciones
            .de_empresa(tenant, negocio.as_deref())
            .await?,
    ))
}

pub async fn corte_manual(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CorteQuery>,
) -> Result<(StatusCode, Json<Vec<Liquidacion>>), ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let liquidaciones = match (params.inicio, params.fin) {
        (Some(inicio), Some(fin)) => {
            state
                .liquidaciones
                .cerrar_periodo(Periodo { inicio, fin })
                .await?
        }
        _ => {
            let periodo = periodo_a_cerrar(hoy_local());
            state.liquidaciones.cerrar_periodo(periodo).await?
        }
    };
    Ok((StatusCode::CREATED, Json(liquidaciones)))
}

pub async fn pagar_liquidacion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<PagarQuery>,
) -> Result<(StatusCode, Json<Liquidacion>), ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let _ = params.estado;
    let liquidacion = state
        .liquidaciones
        .marcar_pagada(&id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Liquidacion", &id))?;
    Ok((StatusCode::CREATED, Json(liquidacion)))
}

pub async fn estado_cuenta_vendedor(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EstadoCuenta>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_VENDEDOR).await?;
    Ok(Json(state.ventas.estado_cuenta(&usuario.user_id).await?))
}

pub async fn estado_cuenta_empresa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<EstadoCuentaEmpresa>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESARIO).await?;
    let Some(tenant) = usuario.tenant_id.as_deref() else {
        return Err(ApiError::no_encontrado("Empresa", "activa"));
    };
    let negocio = negocio_de(&usuario, &params);
    Ok(Json(
        state
            .ventas
            .estado_cuenta_empresa(tenant, negocio.as_deref())
            .await?,
    ))
}

fn negocio_de(usuario: &UsuarioContexto, params: &HashMap<String, String>) -> Option<String> {
    usuario.negocio_id.clone().or_else(|| {
        params
            .get("negocioId")
            .cloned()
            .filter(|valor| !valor.is_empty())
    })
}

pub async fn reportes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ReporteKpi>, ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    Ok(Json(state.reportes.kpis().await?))
}

/// `GET /api/v2/empresarios/me/reportes/negocios`.
pub async fn reportes_negocios(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESARIO).await?;
    let Some(tenant) = usuario.tenant_id.as_deref() else {
        return Err(ApiError::no_encontrado("Empresa", "activa"));
    };
    let negocio = usuario
        .negocio_id
        .clone()
        .or_else(|| params.get("negocioId").cloned().filter(|v| !v.is_empty()));
    let desde = fecha_reporte(params.get("desde"))?;
    let hasta = fecha_reporte(params.get("hasta"))?;
    let filas = state
        .reportes
        .por_negocio(
            tenant,
            negocio.as_deref(),
            desde.as_deref(),
            hasta.as_deref(),
        )
        .await?;
    Ok(Json(Value::Array(filas)))
}

// ── Dispersión diaria (ADR-0053) ──────────────────────────────────────────

pub async fn dispersion_configuracion(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let config = state.dispersion()?.configuracion().await?;
    Ok(Json(
        serde_json::to_value(config).map_err(|_| ApiError::interno())?,
    ))
}

pub async fn guardar_dispersion_configuracion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let config = state.dispersion()?.guardar_configuracion(&dto).await?;
    Ok(Json(
        serde_json::to_value(config).map_err(|_| ApiError::interno())?,
    ))
}

pub async fn dispersion_cuentas(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let cuentas = state.dispersion()?.listar_cuentas().await?;
    Ok(Json(
        serde_json::to_value(cuentas).map_err(|_| ApiError::interno())?,
    ))
}

pub async fn crear_dispersion_cuenta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let cuenta = state.dispersion()?.guardar_cuenta(&dto).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(cuenta).map_err(|_| ApiError::interno())?),
    ))
}

pub async fn dispersion_payouts(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let fecha = params
        .get("fecha")
        .map(|valor| NaiveDate::parse_from_str(valor, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| {
            ApiError::validacion(json!([{"campo": "fecha", "restricciones": ["isDate"]}]))
        })?;
    let estado = params
        .get("estado")
        .map(String::as_str)
        .filter(|valor| !valor.is_empty());
    let payouts = state
        .dispersion()?
        .listar_payouts(fecha, estado, None)
        .await?;
    Ok(Json(
        serde_json::to_value(payouts).map_err(|_| ApiError::interno())?,
    ))
}

pub async fn cerrar_dia_dispersion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let fecha = match params.get("fecha").filter(|valor| !valor.is_empty()) {
        Some(valor) => NaiveDate::parse_from_str(valor, "%Y-%m-%d").map_err(|_| {
            ApiError::validacion(json!([{"campo": "fecha", "restricciones": ["isDate"]}]))
        })?,
        None => hoy_local().pred_opt().unwrap_or_else(|| hoy_local()),
    };
    let resumen = state.dispersion()?.cerrar_dia(fecha).await?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::to_value(resumen).map_err(|_| ApiError::interno())?),
    ))
}

pub async fn enviar_payout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let payout = state.dispersion()?.enviar(&id).await?;
    Ok(Json(
        serde_json::to_value(payout).map_err(|_| ApiError::interno())?,
    ))
}

pub async fn confirmar_payout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(dto): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    let referencia = dto
        .get("referencia_externa")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    let payout = state.dispersion()?.confirmar(&id, referencia).await?;
    Ok(Json(
        serde_json::to_value(payout).map_err(|_| ApiError::interno())?,
    ))
}

pub async fn payouts_empresa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESARIO).await?;
    let Some(tenant) = usuario.tenant_id.as_deref() else {
        return Err(ApiError::no_encontrado("Empresa", "activa"));
    };
    let negocio = negocio_de(&usuario, &params);
    let payouts = state
        .dispersion()?
        .listar_payouts_empresa(tenant, negocio.as_deref())
        .await?;
    Ok(Json(
        serde_json::to_value(payouts).map_err(|_| ApiError::interno())?,
    ))
}

pub async fn payouts_vendedor(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_VENDEDOR).await?;
    let payouts = state
        .dispersion()?
        .listar_payouts_vendedor(&usuario.user_id)
        .await?;
    Ok(Json(
        serde_json::to_value(payouts).map_err(|_| ApiError::interno())?,
    ))
}

fn fecha_reporte(valor: Option<&String>) -> Result<Option<String>, ApiError> {
    let Some(valor) = valor.filter(|valor| !valor.is_empty()) else {
        return Ok(None);
    };
    let valido = chrono::NaiveDate::parse_from_str(valor, "%Y-%m-%d").is_ok()
        || chrono::DateTime::parse_from_rfc3339(valor).is_ok();
    if !valido {
        return Err(ApiError::validacion(json!([
            {"campo": "fecha", "restricciones": ["isDate"]}
        ])));
    }
    Ok(Some(valor.to_string()))
}
