//! Handlers de listas de precio, precios, escalas y resolución.

use super::empresa::de_tenant;
use super::validacion::{self, campo, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::ROLES_CATALOGO;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const CAPACIDAD: &[&str] = &["catalogo_items"];

const LISTA: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Coincide("codigo_ubicacion")]),
    campo("nombre", true, &[Regla::Texto]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("moneda", false, &[Regla::Coincide("moneda_iso")]),
    campo("segmento_codigo", false, &[Regla::Coincide("segmento")]),
    campo("predeterminada", false, &[Regla::Booleano]),
];

const LISTA_ACTUALIZAR: &[ReglaCampo] = &[
    campo("codigo", false, &[Regla::Coincide("codigo_ubicacion")]),
    campo("nombre", false, &[Regla::Texto]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("moneda", false, &[Regla::Coincide("moneda_iso")]),
    campo("segmento_codigo", false, &[Regla::Coincide("segmento")]),
    campo("predeterminada", false, &[Regla::Booleano]),
    campo("activo", false, &[Regla::Booleano]),
];

const PRECIO: &[ReglaCampo] = &[
    campo("item_id", true, &[Regla::Uuid]),
    campo("desde_cantidad", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo("hasta_cantidad", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo("precio_cents", true, &[Regla::Entero, Regla::Min(0.0)]),
];

const PRECIO_ACTUALIZAR: &[ReglaCampo] = &[
    campo("desde_cantidad", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo("hasta_cantidad", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo("precio_cents", false, &[Regla::Entero, Regla::Min(0.0)]),
];

const ESCALA: &[ReglaCampo] = &[
    campo("desde_cantidad", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo("hasta_cantidad", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo("precio_cents", true, &[Regla::Entero, Regla::Min(0.0)]),
];

fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    validacion::validar(esquema, cuerpo)
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn empresa(
    usuario: &crate::auth::UsuarioContexto,
    query: &HashMap<String, String>,
) -> Result<String, ApiError> {
    de_tenant(
        usuario,
        query.get("empresa_id").map(String::as_str),
        "No puedes operar otra empresa.",
    )
}

/// `GET /api/v2/catalog/listas-precio`.
pub async fn listar_listas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state.precios()?.listas(&empresa).await?)
}

/// `POST /api/v2/catalog/listas-precio`.
pub async fn crear_lista(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(LISTA, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    created(state.precios()?.crear_lista(&empresa, &cuerpo).await?)
}

/// `PATCH /api/v2/catalog/listas-precio/:listaId`.
pub async fn actualizar_lista(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lista_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(LISTA_ACTUALIZAR, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .precios()?
        .actualizar_lista(&empresa, &lista_id, &cuerpo)
        .await?)
}

/// `GET /api/v2/catalog/listas-precio/:listaId/precios`.
pub async fn listar_precios(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lista_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state.precios()?.precios(&empresa, &lista_id).await?)
}

/// `POST /api/v2/catalog/listas-precio/:listaId/precios`.
pub async fn crear_precio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(lista_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(PRECIO, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    created(
        state
            .precios()?
            .crear_precio(&empresa, Some(&lista_id), &cuerpo)
            .await?,
    )
}

/// `PATCH /api/v2/catalog/listas-precio/:listaId/precios/:precioId`.
pub async fn actualizar_precio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((lista_id, precio_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(PRECIO_ACTUALIZAR, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .precios()?
        .actualizar_precio(&empresa, Some(&lista_id), &precio_id, &cuerpo)
        .await?)
}

/// `DELETE /api/v2/catalog/listas-precio/:listaId/precios/:precioId`.
pub async fn eliminar_precio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((lista_id, precio_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .precios()?
        .eliminar_precio(&empresa, Some(&lista_id), &precio_id)
        .await?)
}

/// `GET /api/v2/catalog/items/:id/escalas`.
pub async fn listar_escalas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state.precios()?.escalas(&empresa, &item_id).await?)
}

/// `POST /api/v2/catalog/items/:id/escalas`.
pub async fn crear_escala(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(ESCALA, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    let mut datos = cuerpo;
    if let Some(objeto) = datos.as_object_mut() {
        objeto.insert("item_id".into(), json!(item_id));
    }
    created(
        state
            .precios()?
            .crear_precio(&empresa, None, &datos)
            .await?,
    )
}

/// `PATCH /api/v2/catalog/items/:id/escalas/:escalaId`.
pub async fn actualizar_escala(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((item_id, escala_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    validar(PRECIO_ACTUALIZAR, &cuerpo)?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .precios()?
        .actualizar_escala(&empresa, &item_id, &escala_id, &cuerpo)
        .await?)
}

/// `DELETE /api/v2/catalog/items/:id/escalas/:escalaId`.
pub async fn eliminar_escala(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((item_id, escala_id)): Path<(String, String)>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    ok(state
        .precios()?
        .eliminar_escala(&empresa, &item_id, &escala_id)
        .await?)
}

/// `GET /api/v2/catalog/items/:id/precio`.
pub async fn resolver_precio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(item_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAPACIDAD).await?;
    let empresa = empresa(&usuario, &query)?;
    let cantidad = query
        .get("cantidad")
        .cloned()
        .unwrap_or_else(|| "1".to_string());
    let filtros = json!({
        "lista_codigo": query.get("lista"),
        "segmento_codigo": query.get("segmento"),
    });
    ok(state
        .precios()?
        .precio_de(&empresa, &item_id, &Value::String(cantidad), &filtros)
        .await?)
}
