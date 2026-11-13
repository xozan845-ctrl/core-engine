//! Handlers de items unificados (`/api/v2/catalog/items`).

use super::validacion::{self, campo, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::{UsuarioContexto, ROLES_CATALOGO};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const CAPACIDAD: &[&str] = &["catalogo_items"];
const PERMISO_LEER: &[&str] = &["catalogo.leer"];
const PERMISO_ESCRIBIR: &[&str] = &["catalogo.escribir"];

const CREAR_ITEM: &[ReglaCampo] = &[
    campo(
        "tipo",
        true,
        &[Regla::En(&["producto", "servicio", "alquiler"])],
    ),
    campo("codigo", true, &[Regla::Texto]),
    campo("nombre", true, &[Regla::Texto]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("categoria", false, &[Regla::Texto]),
    campo("precio", true, &[Regla::Texto]),
    campo("costo", false, &[Regla::Texto]),
    campo("track_inventory", false, &[Regla::Booleano]),
    campo("stock", false, &[Regla::Entero, Regla::Min(0.0)]),
    campo(
        "duration_minutes",
        false,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(1440.0)],
    ),
    campo("atributos", false, &[Regla::Objeto]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
    campo("parent_item_id", false, &[Regla::Uuid]),
    campo("marca", false, &[Regla::Texto]),
    campo("codigo_barras", false, &[Regla::Texto]),
    campo("unidad", false, &[Regla::Texto]),
    campo("permite_fracciones", false, &[Regla::Booleano]),
    campo("unidad_compra", false, &[Regla::Texto]),
    campo(
        "contenido_cantidad",
        false,
        &[Regla::Entero, Regla::Min(0.0)],
    ),
    campo("contenido_unidad", false, &[Regla::Texto]),
    campo("tipo_precio", false, &[Regla::En(&["fijo", "escala"])]),
    campo("tipo_costo", false, &[Regla::En(&["fijo", "escala"])]),
    campo("stock_minimo", false, &[Regla::Entero, Regla::Min(0.0)]),
    campo("stock_maximo", false, &[Regla::Entero, Regla::Min(0.0)]),
    campo("garantia_meses", false, &[Regla::Entero, Regla::Min(0.0)]),
    campo("es_materia_prima", false, &[Regla::Booleano]),
    campo("ubicacion", false, &[Regla::Texto]),
    campo("empresa_id", false, &[Regla::Uuid]),
];

const CREAR_ALQUILER: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Texto]),
    campo("nombre", true, &[Regla::Texto]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("categoria", false, &[Regla::Texto]),
    campo("precio", true, &[Regla::Texto]),
    campo("costo", false, &[Regla::Texto]),
    campo("atributos", false, &[Regla::Objeto]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
    campo("empresa_id", false, &[Regla::Uuid]),
];

const GUARDAR_ESQUEMA: &[ReglaCampo] = &[
    campo(
        "tipo",
        true,
        &[Regla::En(&["producto", "servicio", "alquiler"])],
    ),
    campo("categoria", true, &[Regla::Texto]),
    campo("esquema", true, &[Regla::Objeto]),
];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    validacion::validar(esquema, cuerpo)
}

/// Empresa de consulta: empresario/rol configurable queda atado a su tenant.
fn empresa_de_consulta(
    usuario: &UsuarioContexto,
    solicitada: Option<&str>,
) -> Result<String, ApiError> {
    if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        let tenant = usuario
            .tenant_id
            .as_deref()
            .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."))?;
        if let Some(solicitada) = solicitada {
            if !solicitada.is_empty() && solicitada != tenant {
                return Err(ApiError::dominio(
                    "EMPRESA_NO_AUTORIZADA",
                    "No puedes consultar otra empresa.",
                ));
            }
        }
        return Ok(tenant.to_string());
    }
    solicitada
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indica empresa_id."))
}

fn empresa_de_escritura(
    usuario: &UsuarioContexto,
    solicitada: Option<&str>,
) -> Result<String, ApiError> {
    if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        return usuario
            .tenant_id
            .clone()
            .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."));
    }
    solicitada
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indica empresa_id."))
}

/// `GET /api/v2/catalog/items`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, PERMISO_LEER, CAPACIDAD).await?;
    let empresa = empresa_de_consulta(&usuario, query.get("empresa_id").map(String::as_str))?;
    let mut filtros = serde_json::Map::new();
    filtros.insert("empresa_id".into(), Value::String(empresa));
    for (clave, valor) in query {
        if clave == "empresa_id" {
            continue;
        }
        filtros.insert(clave, Value::String(valor));
    }
    ok(state.items()?.listar(&Value::Object(filtros)).await?)
}

/// `POST /api/v2/catalog/items`.
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &state,
        &headers,
        ROLES_CATALOGO,
        PERMISO_ESCRIBIR,
        CAPACIDAD,
    )
    .await?;
    validar(CREAR_ITEM, &cuerpo)?;
    let empresa = empresa_de_escritura(&usuario, cuerpo.get("empresa_id").and_then(Value::as_str))?;
    created(state.items()?.crear_item(&cuerpo, &empresa).await?)
}

/// `POST /api/v2/catalog/items/alquileres`.
pub async fn crear_alquiler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &state,
        &headers,
        ROLES_CATALOGO,
        PERMISO_ESCRIBIR,
        CAPACIDAD,
    )
    .await?;
    validar(CREAR_ALQUILER, &cuerpo)?;
    let empresa = empresa_de_escritura(&usuario, cuerpo.get("empresa_id").and_then(Value::as_str))?;
    created(state.items()?.crear_alquiler(&cuerpo, &empresa).await?)
}

/// `POST /api/v2/catalog/items/atributos/esquemas`.
pub async fn guardar_esquema(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &state,
        &headers,
        ROLES_CATALOGO,
        PERMISO_ESCRIBIR,
        CAPACIDAD,
    )
    .await?;
    validar(GUARDAR_ESQUEMA, &cuerpo)?;
    let empresa = if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        usuario.tenant_id.as_deref()
    } else {
        None
    };
    state
        .items()?
        .guardar_esquema(
            empresa,
            cuerpo.get("tipo").unwrap_or(&Value::Null),
            cuerpo
                .get("categoria")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            cuerpo.get("esquema").unwrap_or(&Value::Null),
        )
        .await?;
    created(json!({ "ok": true }))
}

/// `GET /api/v2/catalog/items/atributos/esquemas`.
pub async fn esquemas(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, PERMISO_LEER, CAPACIDAD).await?;
    ok(Value::Array(
        state
            .items()?
            .esquemas(usuario.tenant_id.as_deref())
            .await?,
    ))
}

/// `GET /api/v2/catalog/items/:id`.
pub async fn detalle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, PERMISO_LEER, CAPACIDAD).await?;
    let empresa = empresa_de_consulta(&usuario, query.get("empresa_id").map(String::as_str))?;
    let item = state
        .items()?
        .encontrar_por_id(&id, Some(&empresa))
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Item", &id))?;
    ok(item)
}
