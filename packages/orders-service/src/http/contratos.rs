//! Rutas de contratos con depósito y garantía (Fase P2.2).

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

const MONTO: &str = r"^\d+(\.\d{1,2})?$";
const FECHA: &str = r"^\d{4}-\d{2}-\d{2}$";

const CREAR: &[ReglaCampo] = &[
    campo("order_id", false, &[Regla::Uuid]),
    campo("parte_id", false, &[Regla::Uuid]),
    campo("item_id", false, &[Regla::Uuid]),
    campo(
        "tipo",
        false,
        &[Regla::En(&crate::contratos::TIPOS_CONTRATO)],
    ),
    campo("titulo", true, &[Regla::Texto]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("inicia_en", false, &[Regla::Patron(FECHA)]),
    campo("termina_en", false, &[Regla::Patron(FECHA)]),
    campo("deposito", false, &[Regla::Patron(MONTO)]),
    campo("garantia", false, &[Regla::Patron(MONTO)]),
    campo("moneda", false, &[Regla::Patron(r"^[A-Za-z]{3}$")]),
    campo("metadata", false, &[Regla::Objeto]),
];

const ACTUALIZAR: &[ReglaCampo] = &[
    campo("parte_id", false, &[Regla::Uuid]),
    campo("item_id", false, &[Regla::Uuid]),
    campo(
        "tipo",
        false,
        &[Regla::En(&crate::contratos::TIPOS_CONTRATO)],
    ),
    campo("titulo", false, &[Regla::Texto]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("inicia_en", false, &[Regla::Patron(FECHA)]),
    campo("termina_en", false, &[Regla::Patron(FECHA)]),
    campo("deposito", false, &[Regla::Patron(MONTO)]),
    campo("garantia", false, &[Regla::Patron(MONTO)]),
    campo("moneda", false, &[Regla::Patron(r"^[A-Za-z]{3}$")]),
    campo("metadata", false, &[Regla::Objeto]),
    campo("estado", false, &[Regla::En(&["activo", "cancelado"])]),
];

const FINALIZAR: &[ReglaCampo] = &[
    campo("retenido", false, &[Regla::Patron(MONTO)]),
    campo("motivo", false, &[Regla::Texto, Regla::LongitudMax(500)]),
];

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/contratos", get(listar).post(crear))
        .route("/api/v2/contratos/{id}", get(obtener).patch(actualizar))
        .route("/api/v2/contratos/{id}/finalizar", post(finalizar))
        .route("/api/v2/contratos/{id}/movimientos", get(movimientos))
}

async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["super_admin", "admin", "empresario"], &[])?;
    super::exigir_capacidades(&state, &usuario, &["contratos"]).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let filtros = crate::contratos::FiltrosContrato {
        estado: query(&params, "estado").map(ToString::to_string),
        order_id: query(&params, "order_id").map(ToString::to_string),
        parte_id: query(&params, "parte_id").map(ToString::to_string),
        pagina: query(&params, "pagina").map(ToString::to_string),
        limite: query(&params, "limite").map(ToString::to_string),
    };
    ok(state.contratos()?.listar(&empresa, filtros).await?)
}

async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["super_admin", "admin", "empresario"], &[])?;
    super::exigir_capacidades(&state, &usuario, &["contratos"]).await?;
    validar(CREAR, &cuerpo)?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let input: crate::contratos::CrearContratoInput =
        serde_json::from_value(objeto(&cuerpo).into()).map_err(|_| {
            ApiError::validacion(serde_json::json!([{"campo": "titulo", "restricciones": []}]))
        })?;
    created(
        state
            .contratos()?
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
    let usuario = super::autorizar(&headers, &["super_admin", "admin", "empresario"], &[])?;
    super::exigir_capacidades(&state, &usuario, &["contratos"]).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    ok(state.contratos()?.obtener(&empresa, &id).await?)
}

async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["super_admin", "admin", "empresario"], &[])?;
    super::exigir_capacidades(&state, &usuario, &["contratos"]).await?;
    validar(ACTUALIZAR, &cuerpo)?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let input: crate::contratos::ActualizarContratoInput =
        serde_json::from_value(Value::Object(objeto(&cuerpo))).unwrap_or_default();
    ok(state.contratos()?.actualizar(&empresa, &id, input).await?)
}

async fn finalizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["super_admin", "admin", "empresario"], &[])?;
    super::exigir_capacidades(&state, &usuario, &["contratos"]).await?;
    validar(FINALIZAR, &cuerpo)?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let input: crate::contratos::FinalizarContratoInput =
        serde_json::from_value(Value::Object(objeto(&cuerpo))).unwrap_or_default();
    ok(state
        .contratos()?
        .finalizar(&empresa, &id, &usuario.user_id, input)
        .await?)
}

async fn movimientos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = super::autorizar(&headers, &["super_admin", "admin", "empresario"], &[])?;
    super::exigir_capacidades(&state, &usuario, &["contratos"]).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    ok(state.contratos()?.listar_eventos(&empresa, &id).await?)
}
