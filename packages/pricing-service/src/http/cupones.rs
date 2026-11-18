//! Handlers de cupones (`/api/v2/catalog/cupones`).

use super::empresa::de_operaciones;
use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::ROLES_PRECIOS;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const CAP: &[&str] = &["catalogo_items"];

const CUPON: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Texto, Regla::Longitud(3, 32)]),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo("tipo", true, &[Regla::En(&["porcentaje", "monto_fijo"])]),
    campo("valor", true, &[Regla::Entero, Regla::Min(1.0)]),
    campo(
        "minimo_compra_cents",
        false,
        &[Regla::Entero, Regla::Min(0.0)],
    ),
    campo(
        "maximo_descuento_cents",
        false,
        &[Regla::Entero, Regla::Min(1.0)],
    ),
    campo("usos_maximos", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo("fecha_desde", true, &[Regla::Fecha]),
    campo("fecha_hasta", true, &[Regla::Fecha]),
    campo("activo", false, &[Regla::Booleano]),
];

const CUPON_ACTUALIZAR: &[ReglaCampo] = &[
    campo("codigo", false, &[Regla::Texto, Regla::Longitud(3, 32)]),
    campo("nombre", false, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo("tipo", false, &[Regla::En(&["porcentaje", "monto_fijo"])]),
    campo("valor", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo(
        "minimo_compra_cents",
        false,
        &[Regla::Entero, Regla::Min(0.0)],
    ),
    campo(
        "maximo_descuento_cents",
        false,
        &[Regla::Entero, Regla::Min(1.0)],
    ),
    campo("usos_maximos", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo("fecha_desde", false, &[Regla::Fecha]),
    campo("fecha_hasta", false, &[Regla::Fecha]),
    campo("activo", false, &[Regla::Booleano]),
];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn validar_query(query: &HashMap<String, String>) -> Result<(), ApiError> {
    let mut objeto = serde_json::Map::new();
    for (clave, valor) in query {
        if clave == "pagina" || clave == "limite" {
            objeto.insert(
                clave.clone(),
                valor
                    .trim()
                    .parse::<f64>()
                    .map(|numero| json!(numero))
                    .unwrap_or_else(|_| Value::String(valor.clone())),
            );
        } else if clave == "activo" {
            objeto.insert(clave.clone(), json!(valor == "true"));
        } else {
            objeto.insert(clave.clone(), Value::String(valor.clone()));
        }
    }
    validar(
        &[
            campo("empresa_id", false, &[Regla::Uuid]),
            campo("activo", false, &[Regla::Booleano]),
            campo("pagina", false, &[Regla::Entero, Regla::Min(1.0)]),
            campo(
                "limite",
                false,
                &[Regla::Entero, Regla::Min(1.0), Regla::Max(100.0)],
            ),
        ],
        &Value::Object(objeto),
    )
}

/// `GET /api/v2/catalog/cupones`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    validar_query(&query)?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state.cupones()?.listar(&empresa, &json!(query)).await?)
}

/// `POST /api/v2/catalog/cupones`.
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    validar(CUPON, &cuerpo)?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    created(state.cupones()?.crear(&empresa, &cuerpo).await?)
}

/// `GET /api/v2/catalog/cupones/:id`.
pub async fn detalle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    let cupon = state
        .cupones()?
        .obtener(&id, &empresa)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Cupon", &id))?;
    ok(cupon)
}

/// `PATCH /api/v2/catalog/cupones/:id`.
pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    validar(CUPON_ACTUALIZAR, &cuerpo)?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state.cupones()?.actualizar(&id, &empresa, &cuerpo).await?)
}

/// `DELETE /api/v2/catalog/cupones/:id`.
pub async fn eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    state.cupones()?.eliminar(&id, &empresa).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
