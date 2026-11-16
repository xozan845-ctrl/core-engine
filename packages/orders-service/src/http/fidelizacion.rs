//! Fidelización (capacidad `fidelizacion`): programa, tarjetas, recompensas y
//! canjes. Todo el módulo exige la capacidad activa en la empresa del tenant.

use super::{created, ok, Cuerpo};
use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::Router;
use std::collections::HashMap;

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/fidelizacion/programa",
            get(programa).put(guardar_programa),
        )
        .route(
            "/api/v2/fidelizacion/tarjetas",
            get(listar_tarjetas).post(crear_tarjeta),
        )
        .route("/api/v2/fidelizacion/tarjetas/{id}", get(obtener_tarjeta))
        .route(
            "/api/v2/fidelizacion/tarjetas/{id}/movimientos",
            get(listar_movimientos),
        )
        .route("/api/v2/fidelizacion/tarjetas/{id}/ajustes", post(ajustar))
        .route(
            "/api/v2/fidelizacion/recompensas",
            get(listar_recompensas).post(crear_recompensa),
        )
        .route(
            "/api/v2/fidelizacion/recompensas/{id}",
            patch(actualizar_recompensa),
        )
        .route("/api/v2/fidelizacion/canjes", post(canjear))
}

/// Exige empresario con la capacidad `fidelizacion` activa en su tenant.
async fn contexto(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(UsuarioContexto, String), ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(headers)?;
    actor.exigir_roles(&["empresario"], &["fidelizacion.gestionar"])?;
    let empresa = actor
        .tenant_id
        .clone()
        .filter(|valor| !valor.trim().is_empty())
        .ok_or_else(|| {
            ApiError::dominio(
                "EMPRESA_REQUERIDA",
                "Selecciona una empresa activa para gestionar fidelización.",
            )
        })?;
    let filas = state
        .clientes()?
        .capacidades(
            std::slice::from_ref(&empresa),
            &["fidelizacion".to_string()],
        )
        .await
        .ok_or_else(|| ApiError::servicio_no_disponible("identity-service"))?;
    if !filas.iter().any(|fila| {
        fila.activo && fila.capacidad_codigo == "fidelizacion" && fila.empresa_id == empresa
    }) {
        return Err(ApiError::capacidad_no_activa(&["fidelizacion"], &empresa));
    }
    Ok((actor, empresa))
}

pub async fn programa(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (_, empresa) = contexto(&state, &headers).await?;
    let programa = state.fidelizacion()?.programa(&empresa).await?;
    ok(serde_json::to_value(programa).map_err(|_| ApiError::interno())?)
}

pub async fn guardar_programa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let (_, empresa) = contexto(&state, &headers).await?;
    let programa = state
        .fidelizacion()?
        .guardar_programa(&empresa, &dto)
        .await?;
    ok(serde_json::to_value(programa).map_err(|_| ApiError::interno())?)
}

pub async fn listar_tarjetas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let (_, empresa) = contexto(&state, &headers).await?;
    let parte_id = query
        .get("parte_id")
        .map(String::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    let tarjetas = state
        .fidelizacion()?
        .listar_tarjetas(&empresa, parte_id)
        .await?;
    ok(serde_json::to_value(tarjetas).map_err(|_| ApiError::interno())?)
}

pub async fn obtener_tarjeta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let (_, empresa) = contexto(&state, &headers).await?;
    let tarjeta = state.fidelizacion()?.obtener_tarjeta(&empresa, &id).await?;
    ok(serde_json::to_value(tarjeta).map_err(|_| ApiError::interno())?)
}

pub async fn crear_tarjeta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let (_, empresa) = contexto(&state, &headers).await?;
    let tarjeta = state.fidelizacion()?.crear_tarjeta(&empresa, &dto).await?;
    created(serde_json::to_value(tarjeta).map_err(|_| ApiError::interno())?)
}

pub async fn listar_movimientos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let (_, empresa) = contexto(&state, &headers).await?;
    let limite = query
        .get("limite")
        .and_then(|valor| valor.parse::<i64>().ok())
        .unwrap_or(50);
    let movimientos = state
        .fidelizacion()?
        .listar_movimientos(&empresa, &id, limite)
        .await?;
    ok(serde_json::to_value(movimientos).map_err(|_| ApiError::interno())?)
}

pub async fn ajustar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let (actor, empresa) = contexto(&state, &headers).await?;
    let movimiento = state
        .fidelizacion()?
        .ajustar(&empresa, &id, &dto, &actor.user_id)
        .await?;
    created(serde_json::to_value(movimiento).map_err(|_| ApiError::interno())?)
}

pub async fn listar_recompensas(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (_, empresa) = contexto(&state, &headers).await?;
    let recompensas = state.fidelizacion()?.listar_recompensas(&empresa).await?;
    ok(serde_json::to_value(recompensas).map_err(|_| ApiError::interno())?)
}

pub async fn crear_recompensa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let (_, empresa) = contexto(&state, &headers).await?;
    let recompensa = state
        .fidelizacion()?
        .crear_recompensa(&empresa, &dto)
        .await?;
    created(serde_json::to_value(recompensa).map_err(|_| ApiError::interno())?)
}

pub async fn actualizar_recompensa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let (_, empresa) = contexto(&state, &headers).await?;
    let recompensa = state
        .fidelizacion()?
        .actualizar_recompensa(&empresa, &id, &dto)
        .await?;
    ok(serde_json::to_value(recompensa).map_err(|_| ApiError::interno())?)
}

pub async fn canjear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let (actor, empresa) = contexto(&state, &headers).await?;
    let canje = state
        .fidelizacion()?
        .canjear(&empresa, &dto, &actor.user_id)
        .await?;
    created(serde_json::to_value(canje).map_err(|_| ApiError::interno())?)
}
