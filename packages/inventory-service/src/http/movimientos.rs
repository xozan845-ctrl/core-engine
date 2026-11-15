//! Handlers de movimientos de inventario.

use super::validacion::{self, campo, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::{UsuarioContexto, ROLES_INVENTARIO};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const CAP_STOCK: &[&str] = &["inventario_stock"];

const MOVIMIENTO: &[ReglaCampo] = &[
    campo(
        "tipo",
        true,
        &[Regla::En(&["entrada", "salida", "ajuste", "traslado"])],
    ),
    campo("motivo", false, &[Regla::Texto]),
    campo("referencia", false, &[Regla::Texto]),
    campo("ubicacion_origen", false, &[Regla::Texto]),
    campo("ubicacion_destino", false, &[Regla::Texto]),
    campo("lineas", true, &[Regla::Arreglo]),
];

const LISTAR_MOVIMIENTOS: &[ReglaCampo] = &[
    campo("empresa_id", false, &[Regla::Uuid]),
    campo(
        "tipo",
        false,
        &[Regla::En(&["entrada", "salida", "ajuste", "traslado"])],
    ),
    campo("pagina", false, &[Regla::Numero, Regla::Min(1.0)]),
    campo(
        "limite",
        false,
        &[Regla::Numero, Regla::Min(1.0), Regla::Max(100.0)],
    ),
];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn empresa(usuario: &UsuarioContexto, solicitada: Option<&str>) -> Result<String, ApiError> {
    if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        let tenant = usuario
            .tenant_id
            .as_deref()
            .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."))?;
        if let Some(solicitada) = solicitada {
            if !solicitada.is_empty() && solicitada != tenant {
                return Err(ApiError::dominio(
                    "EMPRESA_NO_AUTORIZADA",
                    "No puedes operar otra empresa.",
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

fn entero_de_query(valor: &str) -> Value {
    match valor.trim().parse::<f64>() {
        Ok(numero) if numero.is_finite() => json!(numero),
        _ => Value::String(valor.to_string()),
    }
}

fn validar_query(esquema: &[ReglaCampo], query: &HashMap<String, String>) -> Result<(), ApiError> {
    let mut objeto = serde_json::Map::new();
    for (clave, valor) in query {
        if clave == "pagina" || clave == "limite" {
            objeto.insert(clave.clone(), entero_de_query(valor));
        } else {
            objeto.insert(clave.clone(), Value::String(valor.clone()));
        }
    }
    validacion::validar(esquema, &Value::Object(objeto))
}

fn validar_lineas_movimiento(valor: &Value) -> Result<(), ApiError> {
    let Some(lineas) = valor.as_array() else {
        return Err(ApiError::validacion(
            json!([{ "campo": "lineas", "restricciones": ["isArray"] }]),
        ));
    };
    if lineas.is_empty() {
        return Err(ApiError::validacion(
            json!([{ "campo": "lineas", "restricciones": ["arrayMinSize"] }]),
        ));
    }
    for linea in lineas {
        let valido = linea.as_object().is_some_and(|objeto| {
            objeto
                .get("sku")
                .and_then(Value::as_str)
                .is_some_and(|sku| !sku.is_empty())
                && objeto
                    .get("cantidad")
                    .and_then(Value::as_i64)
                    .is_some_and(|cantidad| cantidad >= 0)
        });
        if !valido {
            return Err(ApiError::validacion(
                json!([{ "campo": "lineas", "restricciones": [] }]),
            ));
        }
    }
    Ok(())
}

/// `GET /api/v2/catalog/inventario/movimientos`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_INVENTARIO, &[], CAP_STOCK).await?;
    validar_query(LISTAR_MOVIMIENTOS, &query)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state.movimientos()?.listar(&empresa, &json!(query)).await?)
}

/// `POST /api/v2/catalog/inventario/movimientos`.
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_INVENTARIO, &[], CAP_STOCK).await?;
    validacion::validar(MOVIMIENTO, &cuerpo)?;
    validar_lineas_movimiento(cuerpo.get("lineas").unwrap_or(&Value::Null))?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    created(
        state
            .movimientos()?
            .crear(&empresa, &cuerpo, Some(&usuario.user_id))
            .await?,
    )
}
