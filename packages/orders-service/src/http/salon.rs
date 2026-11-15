//! Rutas de salón: mesas y comandas de preparación (Fase P2.5).

use super::validacion::{campo, objeto, validar, Regla, ReglaCampo};
use super::{created, ok, query, Cuerpo};
use crate::errors::ApiError;
use crate::salon::ComandaItem;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, patch};
use axum::Router;
use serde_json::{json, Value};
use std::collections::HashMap;

const CODIGO_MESA: &str = r"^[A-Za-z0-9_-]{1,20}$";

const CREAR_MESA: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Patron(CODIGO_MESA)]),
    campo("nombre", true, &[Regla::Texto]),
    campo(
        "capacidad",
        false,
        &[Regla::Entero, Regla::Min(1), Regla::Max(999)],
    ),
    campo("zona", false, &[Regla::Texto]),
    campo("estado", false, &[Regla::En(&crate::salon::ESTADOS_MESA)]),
];

const ACTUALIZAR_MESA: &[ReglaCampo] = &[
    campo("codigo", false, &[Regla::Patron(CODIGO_MESA)]),
    campo("nombre", false, &[Regla::Texto]),
    campo(
        "capacidad",
        false,
        &[Regla::Entero, Regla::Min(1), Regla::Max(999)],
    ),
    campo("zona", false, &[Regla::Texto]),
    campo("estado", false, &[Regla::En(&crate::salon::ESTADOS_MESA)]),
];

const CREAR_COMANDA: &[ReglaCampo] = &[
    campo("order_id", false, &[Regla::Uuid]),
    campo("mesa_id", false, &[Regla::Uuid]),
    campo("estacion", true, &[Regla::Texto]),
    campo("notas", false, &[Regla::Texto]),
    campo("items", false, &[Regla::Arreglo]),
];

const ACTUALIZAR_COMANDA: &[ReglaCampo] = &[
    campo(
        "estado",
        false,
        &[Regla::En(&crate::salon::ESTADOS_COMANDA)],
    ),
    campo("notas", false, &[Regla::Texto]),
];

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/mesas", get(listar_mesas).post(crear_mesa))
        .route("/api/v2/mesas/{id}", patch(actualizar_mesa))
        .route("/api/v2/comandas", get(listar_comandas).post(crear_comanda))
        .route("/api/v2/comandas/{id}", patch(actualizar_comanda))
}

async fn listar_mesas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let filtros = crate::salon::FiltrosMesa {
        zona: query(&params, "zona").map(ToString::to_string),
        estado: query(&params, "estado").map(ToString::to_string),
    };
    ok(state.salon()?.listar_mesas(&empresa, filtros).await?)
}

async fn crear_mesa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    validar(CREAR_MESA, &cuerpo)?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let input: crate::salon::CrearMesaInput =
        serde_json::from_value(Value::Object(objeto(&cuerpo)))
            .map_err(|_| ApiError::validacion(json!([{"campo": "codigo", "restricciones": []}])))?;
    created(state.salon()?.crear_mesa(&empresa, input).await?)
}

async fn actualizar_mesa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    validar(ACTUALIZAR_MESA, &cuerpo)?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let input: crate::salon::ActualizarMesaInput =
        serde_json::from_value(Value::Object(objeto(&cuerpo))).unwrap_or_default();
    ok(state.salon()?.actualizar_mesa(&empresa, &id, input).await?)
}

async fn listar_comandas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let filtros = crate::salon::FiltrosComanda {
        estado: query(&params, "estado").map(ToString::to_string),
        estacion: query(&params, "estacion").map(ToString::to_string),
        mesa_id: query(&params, "mesa_id").map(ToString::to_string),
    };
    ok(state.salon()?.listar_comandas(&empresa, filtros).await?)
}

async fn crear_comanda(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    validar(CREAR_COMANDA, &cuerpo)?;
    let datos = objeto(&cuerpo);
    let items = match datos.get("items") {
        None | Some(Value::Null) => None,
        Some(Value::Array(lista)) => {
            let mut salida = Vec::new();
            for item in lista {
                let Some(objeto) = item.as_object() else {
                    return Err(validacion_items());
                };
                let sku = objeto
                    .get("sku")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let nombre = objeto
                    .get("nombre")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let cantidad = objeto.get("cantidad").and_then(Value::as_i64).unwrap_or(0);
                let notas = objeto
                    .get("notas")
                    .and_then(Value::as_str)
                    .map(|valor| valor.trim().to_string())
                    .filter(|valor| !valor.is_empty());
                if sku.is_empty() || nombre.is_empty() || cantidad < 1 {
                    return Err(validacion_items());
                }
                salida.push(ComandaItem {
                    sku: sku.to_string(),
                    nombre: nombre.to_string(),
                    cantidad,
                    notas,
                });
            }
            Some(salida)
        }
        Some(_) => return Err(validacion_items()),
    };
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    let input = crate::salon::CrearComandaInput {
        order_id: datos
            .get("order_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        mesa_id: datos
            .get("mesa_id")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        estacion: datos
            .get("estacion")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        notas: datos
            .get("notas")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        items,
    };
    created(state.salon()?.crear_comanda(&empresa, input).await?)
}

async fn actualizar_comanda(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    validar(ACTUALIZAR_COMANDA, &cuerpo)?;
    let datos = objeto(&cuerpo);
    let input = crate::salon::ActualizarComandaInput {
        estado: datos
            .get("estado")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        notas: datos
            .get("notas")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    };
    let empresa = usuario.empresa_de(query(&params, "empresa_id"))?;
    ok(state
        .salon()?
        .actualizar_comanda(&empresa, &id, input)
        .await?)
}

fn validacion_items() -> ApiError {
    ApiError::validacion(json!([{"campo": "items", "restricciones": []}]))
}

async fn autorizar(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::auth::UsuarioContexto, ApiError> {
    let usuario = super::autorizar(headers, &["super_admin", "admin", "empresario"], &[])?;
    super::exigir_capacidades(state, &usuario, &["mesas"]).await?;
    Ok(usuario)
}
