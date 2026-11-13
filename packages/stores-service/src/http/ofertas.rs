//! Handlers HTTP de ofertas y de la tienda pública.

use super::validacion::{self, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::{CAPACIDAD_CATALOGO_ITEMS, ROLES_VENDEDOR};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

const PERMISO_GESTIONAR: &[&str] = &["tiendas.ofertas.gestionar"];
const PERMISO_LEER: &[&str] = &["tiendas.ofertas.leer"];

fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    validacion::validar(esquema, cuerpo)
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

/// `POST /api/v2/vendedores/productos`.
pub async fn publicar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &state,
        &headers,
        ROLES_VENDEDOR,
        PERMISO_GESTIONAR,
        &[CAPACIDAD_CATALOGO_ITEMS],
    )
    .await?;
    validar(validacion::PUBLICAR_ITEM, &cuerpo)?;
    let tienda_id = match cuerpo.get("tienda_id").and_then(Value::as_str) {
        Some(identificador) => Some(
            state
                .tiendas()?
                .encontrar_por_id(identificador)
                .await?
                .ok_or_else(|| ApiError::no_encontrado("Tienda", identificador))?
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        ),
        None => state
            .tiendas()?
            .de_vendedor(&usuario.user_id)
            .await?
            .and_then(|tienda| tienda.get("id").and_then(Value::as_str).map(str::to_string)),
    };
    let Some(tienda_id) = tienda_id else {
        return Err(ApiError::no_encontrado(
            "Tienda del vendedor",
            &usuario.user_id,
        ));
    };
    let margen = cuerpo
        .get("margen")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let moneda = cuerpo
        .get("moneda")
        .and_then(Value::as_str)
        .unwrap_or("NIO");
    let oferta = state
        .ofertas()?
        .publicar_item(
            &usuario.user_id,
            &tienda_id,
            cuerpo
                .get("item_id")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            margen,
            moneda,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(oferta)).into_response())
}

/// `PATCH /api/v2/vendedores/productos/:id`.
pub async fn cambiar_margen(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &state,
        &headers,
        ROLES_VENDEDOR,
        PERMISO_GESTIONAR,
        &[CAPACIDAD_CATALOGO_ITEMS],
    )
    .await?;
    validar(validacion::CAMBIAR_MARGEN, &cuerpo)?;
    ok(state
        .ofertas()?
        .cambiar_margen(
            &id,
            &usuario.user_id,
            cuerpo
                .get("margen")
                .and_then(Value::as_i64)
                .unwrap_or_default(),
        )
        .await?)
}

/// `GET /api/v2/vendedores/me/ofertas`.
pub async fn mis_ofertas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_VENDEDOR, PERMISO_LEER, &[]).await?;
    ok(state
        .ofertas()?
        .de_vendedor(
            &usuario.user_id,
            query.get("pagina").map(String::as_str),
            query.get("limite").map(String::as_str),
        )
        .await?)
}

/// Consulta de ids separados por coma para la superficie interna.
#[derive(Debug, Default, Deserialize)]
pub struct LoteIds {
    #[serde(default)]
    pub ids: String,
}

pub fn ids_de_lote(valor: &str) -> Vec<String> {
    valor
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}
