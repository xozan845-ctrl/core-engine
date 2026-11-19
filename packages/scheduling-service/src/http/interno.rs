//! Endpoints internos de ciclo de vida del tenant.

use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;
use serde_json::{json, Value};

fn exigir_clave(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let esperada = state.config.internal_api_key.as_deref().unwrap_or("");
    let recibida = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("");
    if esperada.is_empty() || recibida != esperada {
        return Err(ApiError::acceso_denegado("Clave interna invalida."));
    }
    Ok(())
}

pub async fn exportar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<crate::lifecycle::TenantRequest>,
) -> Result<Json<Value>, ApiError> {
    exigir_clave(&state, &headers)?;
    let datos = crate::lifecycle::exportar(state.db()?.pool(), &dto)
        .await
        .map_err(|_| ApiError::interno())?;
    Ok(Json(
        json!({ "servicio": "scheduling", "estado": "completado", "datos": datos }),
    ))
}

pub async fn purgar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<crate::lifecycle::TenantRequest>,
) -> Result<Json<Value>, ApiError> {
    exigir_clave(&state, &headers)?;
    let detalle = crate::lifecycle::purgar(state.db()?.pool(), &dto)
        .await
        .map_err(|_| ApiError::interno())?;
    Ok(Json(
        json!({ "servicio": "scheduling", "estado": "completado", "detalle": detalle }),
    ))
}
