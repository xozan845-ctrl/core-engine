//! Handlers de promociones (`/api/v2/catalog/promociones`).

use super::empresa::de_operaciones;
use super::validacion::{self, campo, Regla, ReglaCampo};
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

const PROMOCION: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 200)]),
    campo(
        "tipo",
        true,
        &[Regla::En(&[
            "2x1",
            "descuento_cantidad",
            "descuento_porcentaje",
            "regalo",
        ])],
    ),
    campo("item_id", false, &[Regla::Uuid]),
    campo("sku", false, &[Regla::Texto]),
    campo(
        "cantidad_requerida",
        false,
        &[Regla::Entero, Regla::Min(1.0)],
    ),
    campo("cantidad_gratis", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo(
        "descuento_porcentaje",
        false,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(100.0)],
    ),
    campo("fecha_desde", true, &[Regla::Fecha]),
    campo("fecha_hasta", true, &[Regla::Fecha]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
];

const PROMOCION_ACTUALIZAR: &[ReglaCampo] = &[
    campo("nombre", false, &[Regla::Texto, Regla::Longitud(2, 200)]),
    campo(
        "tipo",
        false,
        &[Regla::En(&[
            "2x1",
            "descuento_cantidad",
            "descuento_porcentaje",
            "regalo",
        ])],
    ),
    campo("item_id", false, &[Regla::Uuid]),
    campo("sku", false, &[Regla::Texto]),
    campo(
        "cantidad_requerida",
        false,
        &[Regla::Entero, Regla::Min(1.0)],
    ),
    campo("cantidad_gratis", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo(
        "descuento_porcentaje",
        false,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(100.0)],
    ),
    campo("fecha_desde", false, &[Regla::Fecha]),
    campo("fecha_hasta", false, &[Regla::Fecha]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn sin_contenido() -> Result<Response, ApiError> {
    Ok(StatusCode::NO_CONTENT.into_response())
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
        } else {
            objeto.insert(clave.clone(), Value::String(valor.clone()));
        }
    }
    validacion::validar(
        &[
            campo("empresa_id", false, &[Regla::Uuid]),
            campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
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

/// `GET /api/v2/catalog/promociones`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    validar_query(&query)?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state.promociones()?.listar(&empresa, &json!(query)).await?)
}

/// `POST /api/v2/catalog/promociones`.
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    validacion::validar(PROMOCION, &cuerpo)?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    created(state.promociones()?.crear(&empresa, &cuerpo).await?)
}

/// `GET /api/v2/catalog/promociones/:id`.
pub async fn detalle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    let promocion = state
        .promociones()?
        .obtener(&id, &empresa)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Promoción", &id))?;
    ok(promocion)
}

/// `PATCH /api/v2/catalog/promociones/:id`.
pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    validacion::validar(PROMOCION_ACTUALIZAR, &cuerpo)?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .promociones()?
        .actualizar(&id, &empresa, &cuerpo)
        .await?)
}

/// `DELETE /api/v2/catalog/promociones/:id`.
pub async fn eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_PRECIOS, &[], CAP).await?;
    let empresa = de_operaciones(&usuario, query.get("empresa_id").map(String::as_str))?;
    state.promociones()?.eliminar(&id, &empresa).await?;
    sin_contenido()
}
