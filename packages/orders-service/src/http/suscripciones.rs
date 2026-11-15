//! Rutas de suscripciones y órdenes recurrentes (Fase P2.3).

use super::validacion::{campo, objeto, validar, Regla, ReglaCampo};
use super::{created, ok, query, Cuerpo};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use serde_json::Value;
use std::collections::HashMap;

const FECHA: &str = r"^\d{4}-\d{2}-\d{2}$";

const CREAR: &[ReglaCampo] = &[
    campo("oferta_id", true, &[Regla::Uuid]),
    campo("item_id", false, &[Regla::Uuid]),
    campo(
        "cantidad",
        false,
        &[Regla::Entero, Regla::Min(1), Regla::Max(99)],
    ),
    campo("comprador_id", false, &[Regla::Uuid]),
    campo("cliente_parte_id", false, &[Regla::Uuid]),
    campo("metodo_pago", false, &[Regla::En(&["simulado", "credito"])]),
    campo(
        "fulfillment",
        false,
        &[Regla::En(&crate::suscripciones::FULFILLMENTS_SUSCRIPCION)],
    ),
    campo(
        "intervalo",
        true,
        &[Regla::En(&crate::suscripciones::INTERVALOS)],
    ),
    campo("proxima_ejecucion", false, &[Regla::Patron(FECHA)]),
    campo("fin_en", false, &[Regla::Patron(FECHA)]),
    campo("metadata", false, &[Regla::Objeto]),
];

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/suscripciones", get(listar).post(crear))
        .route("/api/v2/suscripciones/{id}", get(obtener))
        .route("/api/v2/suscripciones/{id}/pausar", post(pausar))
        .route("/api/v2/suscripciones/{id}/reanudar", post(reanudar))
        .route("/api/v2/suscripciones/{id}/cancelar", post(cancelar))
        .route("/api/v2/suscripciones/{id}/renovar", post(renovar))
        .route("/api/v2/suscripciones/{id}/ejecuciones", get(ejecuciones))
}

async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let filtros = crate::suscripciones::FiltrosSuscripcion {
        estado: query(&params, "estado").map(ToString::to_string),
        comprador_id: query(&params, "comprador_id").map(ToString::to_string),
        limite: query(&params, "limite").map(ToString::to_string),
    };
    ok(state.suscripciones()?.listar(&empresa, filtros).await?)
}

async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    validar(CREAR, &cuerpo)?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let input: crate::suscripciones::CrearSuscripcionInput =
        serde_json::from_value(Value::Object(objeto(&cuerpo))).map_err(|_| {
            ApiError::validacion(serde_json::json!([{"campo": "oferta_id", "restricciones": []}]))
        })?;
    created(
        state
            .suscripciones()?
            .crear(&empresa, &usuario.user_id, input)
            .await?,
    )
}

async fn obtener(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    ok(state.suscripciones()?.obtener(&empresa, &id).await?)
}

async fn pausar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    ok(state.suscripciones()?.pausar(&empresa, &id).await?)
}

async fn reanudar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    ok(state.suscripciones()?.reanudar(&empresa, &id).await?)
}

async fn cancelar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    ok(state.suscripciones()?.cancelar(&empresa, &id).await?)
}

async fn renovar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    ok(state.suscripciones()?.renovar(&empresa, &id, true).await?)
}

async fn ejecuciones(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    ok(state
        .suscripciones()?
        .listar_ejecuciones(&empresa, &id)
        .await?)
}

async fn autorizar(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::auth::UsuarioContexto, ApiError> {
    let usuario = super::autorizar(headers, &["super_admin", "admin", "empresario"], &[])?;
    super::exigir_capacidades(state, &usuario, &["suscripciones"]).await?;
    Ok(usuario)
}
