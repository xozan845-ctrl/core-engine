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
use serde::Deserialize;

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
) -> Result<Json<Vec<LiquidacionEmpresa>>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESARIO).await?;
    let Some(tenant) = usuario.tenant_id.as_deref() else {
        return Err(ApiError::no_encontrado("Empresa", "activa"));
    };
    Ok(Json(state.liquidaciones.de_empresa(tenant).await?))
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
) -> Result<Json<EstadoCuentaEmpresa>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESARIO).await?;
    let Some(tenant) = usuario.tenant_id.as_deref() else {
        return Err(ApiError::no_encontrado("Empresa", "activa"));
    };
    Ok(Json(state.ventas.estado_cuenta_empresa(tenant).await?))
}

pub async fn reportes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ReporteKpi>, ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_ADMIN).await?;
    Ok(Json(state.reportes.kpis().await?))
}
