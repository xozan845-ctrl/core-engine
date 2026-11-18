//! Reseñas de comprador y moderación.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch};
use axum::{Json, Router};
use serde_json::Value;
use std::collections::HashMap;

const RESENA: &[ReglaCampo] = &[
    campo("item_id", true, &[Regla::Uuid]),
    campo("order_id", false, &[Regla::Uuid]),
    campo(
        "calificacion",
        true,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(5.0)],
    ),
    campo("titulo", false, &[Regla::Texto, Regla::Longitud(0, 120)]),
    campo(
        "comentario",
        false,
        &[Regla::Texto, Regla::Longitud(0, 2000)],
    ),
];

const RESENA_ACTUALIZAR: &[ReglaCampo] = &[
    campo(
        "calificacion",
        false,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(5.0)],
    ),
    campo("titulo", false, &[Regla::Texto, Regla::Longitud(0, 120)]),
    campo(
        "comentario",
        false,
        &[Regla::Texto, Regla::Longitud(0, 2000)],
    ),
];

const MODERAR: &[ReglaCampo] = &[campo(
    "estado",
    true,
    &[Regla::En(&["pendiente", "publicada", "rechazada"])],
)];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v2/tienda/items/{item_id}/resenas",
            get(listar_publicas),
        )
        .route("/api/v2/comprador/resenas", get(listar_propias).post(crear))
        .route(
            "/api/v2/comprador/resenas/{id}",
            patch(actualizar).delete(eliminar),
        )
        .route("/api/v2/admin/resenas/{id}", patch(moderar))
}

async fn listar_publicas(
    State(state): State<AppState>,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let resenas = state
        .resenas()?
        .listar_publicas(
            &item_id,
            query.get("pagina").map(String::as_str),
            query.get("limite").map(String::as_str),
        )
        .await?;
    ok(resenas)
}

async fn listar_propias(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &["comprador"], &[], &[]).await?;
    ok(
        serde_json::to_value(state.resenas()?.listar_comprador(&usuario.user_id).await?)
            .map_err(|_| ApiError::interno())?,
    )
}

async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &["comprador"], &[], &[]).await?;
    validar(RESENA, &dto)?;
    created(state.resenas()?.crear(&usuario.user_id, &dto).await?)
}

async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &["comprador"], &[], &[]).await?;
    validar(RESENA_ACTUALIZAR, &dto)?;
    ok(state
        .resenas()?
        .actualizar(&id, &usuario.user_id, &dto)
        .await?)
}

async fn eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &["comprador"], &[], &[]).await?;
    state.resenas()?.eliminar(&id, &usuario.user_id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn moderar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    autorizar(&state, &headers, &["super_admin", "admin"], &[], &[]).await?;
    validar(MODERAR, &dto)?;
    let estado = dto.get("estado").and_then(Value::as_str).unwrap_or("");
    ok(state.resenas()?.moderar(&id, estado).await?)
}
