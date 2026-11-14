//! Handlers de packs, verticales y modelos de operación.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::dominio::packs::{
    CrearPack, ModeloOperacion, Pack, PackCapacidadInput, PackModeloInput, VerticalOperacion,
};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const MODELO: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo(
        "descripcion",
        false,
        &[Regla::Texto, Regla::MaxLongitud(500)],
    ),
    campo("config", false, &[Regla::Objeto]),
    campo("activo", false, &[Regla::Booleano]),
];

const VERTICAL: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo(
        "modelo_operacion",
        true,
        &[Regla::Texto, Regla::Longitud(2, 160)],
    ),
    campo("activo", false, &[Regla::Booleano]),
];

const PACK: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("version", true, &[Regla::Entero, Regla::Min(1.0)]),
    campo(
        "vertical_codigo",
        true,
        &[Regla::Texto, Regla::Coincide("segmento")],
    ),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo(
        "descripcion",
        false,
        &[Regla::Texto, Regla::MaxLongitud(500)],
    ),
    campo("config", false, &[Regla::Objeto]),
];

const VERTICAL_QUERY: &[ReglaCampo] = &[campo(
    "vertical_id",
    false,
    &[Regla::Texto, Regla::Coincide("segmento")],
)];

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn valores<T: serde::Serialize>(valor: T) -> Result<Value, ApiError> {
    serde_json::to_value(valor).map_err(|_| ApiError::interno())
}

fn actor(headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    UsuarioContexto::desde_headers_o_error(headers)
}

/// GET /api/v2/packs?vertical_id=...
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    actor(&headers)?.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let vertical = query
        .get("vertical_id")
        .map(|valor| valor.trim().to_lowercase())
        .filter(|valor| !valor.is_empty());
    if let Some(vertical) = vertical.as_deref() {
        validar(VERTICAL_QUERY, &json!({ "vertical_id": vertical }))?;
    }
    let packs = state.servicios()?.packs.listar(vertical.as_deref()).await?;
    ok(valores(packs)?)
}

/// GET /api/v2/packs/modelos-operacion
pub async fn listar_modelos(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    actor(&headers)?.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let modelos: Vec<ModeloOperacion> = state.servicios()?.packs.listar_modelos().await?;
    ok(valores(modelos)?)
}

/// GET /api/v2/packs/verticales
pub async fn listar_verticales(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    actor(&headers)?.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let verticales: Vec<VerticalOperacion> = state.servicios()?.packs.listar_verticales().await?;
    ok(valores(verticales)?)
}

/// POST /api/v2/packs/modelos-operacion
pub async fn crear_modelo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(MODELO, &dto)?;
    actor(&headers)?.exigir_roles(&["super_admin", "admin"], &[])?;
    let modelo = state
        .servicios()?
        .packs
        .crear_modelo(
            dto.get("codigo")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            dto.get("nombre")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            dto.get("descripcion").and_then(Value::as_str),
            dto.get("config").cloned(),
            dto.get("activo").and_then(Value::as_bool),
        )
        .await?;
    created(valores(modelo)?)
}

/// POST /api/v2/packs/verticales
pub async fn crear_vertical(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(VERTICAL, &dto)?;
    actor(&headers)?.exigir_roles(&["super_admin", "admin"], &[])?;
    let vertical = state
        .servicios()?
        .packs
        .crear_vertical(
            dto.get("codigo")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            dto.get("nombre")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            dto.get("modelo_operacion")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            dto.get("activo").and_then(Value::as_bool),
        )
        .await?;
    created(valores(vertical)?)
}

/// POST /api/v2/packs
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(PACK, &dto)?;
    actor(&headers)?.exigir_roles(&["super_admin", "admin"], &[])?;
    let modelos = items(&dto, "modelos")?;
    let capacidades = items(&dto, "capacidades")?;
    let pack = state
        .servicios()?
        .packs
        .crear(CrearPack {
            codigo: dto
                .get("codigo")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            version: dto.get("version").and_then(Value::as_i64).unwrap_or(0),
            vertical_codigo: dto
                .get("vertical_codigo")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            nombre: dto
                .get("nombre")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            descripcion: dto
                .get("descripcion")
                .and_then(Value::as_str)
                .map(str::to_string),
            config: dto.get("config").cloned(),
            modelos: modelos
                .into_iter()
                .map(|item| PackModeloInput {
                    codigo: item.codigo,
                    config: item.config,
                    activo: item.activo,
                })
                .collect(),
            capacidades: capacidades
                .into_iter()
                .map(|item| PackCapacidadInput {
                    codigo: item.codigo,
                    config: item.config,
                    activo: item.activo,
                })
                .collect(),
        })
        .await?;
    created(valores(pack)?)
}

/// GET /api/v2/packs/:codigo/:version
pub async fn obtener(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((codigo, version)): Path<(String, i64)>,
) -> Result<Response, ApiError> {
    actor(&headers)?.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let pack: Pack = state
        .servicios()?
        .packs
        .obtener(&codigo.trim().to_lowercase(), version)
        .await?;
    ok(valores(pack)?)
}

#[derive(Debug, Clone)]
struct Item {
    codigo: String,
    config: Option<Value>,
    activo: Option<bool>,
}

fn items(dto: &Value, clave: &str) -> Result<Vec<Item>, ApiError> {
    let Some(valor) = dto.get(clave) else {
        return Ok(Vec::new());
    };
    if valor.is_null() {
        return Ok(Vec::new());
    }
    let Some(lista) = valor.as_array() else {
        return Err(ApiError::validacion(json!([
            {"campo": clave, "restricciones": ["isArray"]}
        ])));
    };
    lista
        .iter()
        .enumerate()
        .map(|(indice, item)| {
            let Some(objeto) = item.as_object() else {
                return Err(ApiError::validacion(json!([
                    {"campo": clave, "restricciones": ["isObject"]}
                ])));
            };
            let codigo = objeto
                .get("codigo")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|valor| !valor.is_empty())
                .ok_or_else(|| {
                    ApiError::validacion(json!([
                        {"campo": format!("{clave}[{indice}].codigo"), "restricciones": ["isString"]}
                    ]))
                })?;
            if !codigo_valido(codigo) {
                return Err(ApiError::validacion(json!([
                    {"campo": format!("{clave}[{indice}].codigo"), "restricciones": ["matches"]}
                ])));
            }
            let config = objeto.get("config").filter(|valor| !valor.is_null()).cloned();
            if let Some(config) = &config {
                if !config.is_object() {
                    return Err(ApiError::validacion(json!([
                        {"campo": format!("{clave}[{indice}].config"), "restricciones": ["isObject"]}
                    ])));
                }
            }
            let activo = match objeto.get("activo") {
                None | Some(Value::Null) => None,
                Some(Value::Bool(valor)) => Some(*valor),
                Some(_) => {
                    return Err(ApiError::validacion(json!([
                        {"campo": format!("{clave}[{indice}].activo"), "restricciones": ["isBoolean"]}
                    ])))
                }
            };
            Ok(Item {
                codigo: codigo.to_string(),
                config,
                activo,
            })
        })
        .collect()
}

fn codigo_valido(valor: &str) -> bool {
    let mut chars = valor.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=60).contains(&valor.len())
        && valor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_los_items_anidados_como_class_validator() {
        let dto = json!({
            "modelos": [
                {"codigo": "modelo_1", "config": {}, "activo": true}
            ]
        });
        assert_eq!(items(&dto, "modelos").expect("modelos").len(), 1);
        assert!(items(&json!({"modelos": "no"}), "modelos").is_err());
        assert!(items(&json!({"modelos": [{"codigo": "MAL"}]}), "modelos").is_err());
        assert!(items(
            &json!({"modelos": [{"codigo": "ok", "config": []}]}),
            "modelos"
        )
        .is_err());
    }

    #[test]
    fn debe_exigir_un_pack_con_prefijo() {
        assert!(validar(
            PACK,
            &json!({
                "codigo": "pack_retail",
                "version": 1,
                "vertical_codigo": "retail",
                "nombre": "Retail"
            })
        )
        .is_ok());
        assert!(validar(
            PACK,
            &json!({
                "codigo": "retail",
                "version": 1,
                "vertical_codigo": "retail",
                "nombre": "Retail"
            })
        )
        .is_ok());
    }
}
