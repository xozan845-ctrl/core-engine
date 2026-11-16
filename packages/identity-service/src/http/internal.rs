//! API interna (servicio→servicio) protegida por `x-internal-key`.

use super::Cuerpo;
use crate::dominio::slug::es_uuid;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

fn clave_valida(headers: &HeaderMap, esperada: Option<&str>) -> Result<(), ApiError> {
    let recibida = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("");
    let esperada = esperada.unwrap_or("");
    if esperada.is_empty() || recibida != esperada {
        return Err(ApiError::acceso_denegado("Clave interna invalida."));
    }
    Ok(())
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

/// GET /internal/usuarios/:id
pub async fn usuario(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let usuario = state
        .servicios()?
        .usuarios
        .encontrar_por_id(&id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Usuario", &id))?;
    ok(serde_json::to_value(usuario).map_err(|_| ApiError::interno())?)
}

/// GET /internal/vendedores
pub async fn vendedores(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let vendedores = state.servicios()?.usuarios.listar_vendedores().await?;
    ok(serde_json::to_value(vendedores).map_err(|_| ApiError::interno())?)
}

/// GET /internal/contar-vendedores
pub async fn contar_vendedores(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let total = state.servicios()?.usuarios.contar_vendedores().await?;
    ok(json!({ "vendedores": total }))
}

/// GET /internal/empresas/:id
pub async fn empresa(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let empresa = state
        .servicios()?
        .empresas
        .obtener(&id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", &id))?;
    ok(empresa)
}

/// GET /internal/empresas/:id/configuracion
pub async fn empresa_configuracion(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let configuracion = state
        .servicios()?
        .configuracion_operativa
        .obtener(&id)
        .await?;
    ok(serde_json::to_value(configuracion).map_err(|_| ApiError::interno())?)
}

/// GET /internal/empresas/:id/tema
pub async fn empresa_tema(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let tema = state.servicios()?.empresas.tema_overrides(&id).await?;
    ok(tema)
}

/// GET /internal/configuracion-sitio/tema
pub async fn plataforma_tema(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let tema = state.servicios()?.configuracion.tema_overrides().await?;
    ok(tema)
}

/// POST /internal/empresas/capacidades
pub async fn capacidades_empresas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let empresa_ids = arreglo_texto(&dto, "empresa_ids")?;
    let capacidades = arreglo_texto(&dto, "capacidades")?;
    let negocio_id = dto
        .get("negocio_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty());
    for empresa_id in &empresa_ids {
        if !es_uuid(empresa_id) {
            return Err(ApiError::validacion(json!([
                {"campo": "empresa_ids", "restricciones": ["isUUID"]}
            ])));
        }
    }
    if let Some(negocio_id) = negocio_id {
        if !es_uuid(negocio_id) {
            return Err(ApiError::validacion(json!([
                {"campo": "negocio_id", "restricciones": ["isUUID"]}
            ])));
        }
    }
    let filas = state
        .servicios()?
        .capacidades
        .activas_de_empresas(&empresa_ids, &capacidades, negocio_id)
        .await?;
    ok(json!({ "capacidades": filas }))
}

/// POST /internal/extensiones/valores
pub async fn valores_extensiones(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let empresa_id = texto(&dto, "empresa_id")?;
    let recurso = texto(&dto, "recurso")?;
    let recurso_ids = arreglo_texto(&dto, "recurso_ids")?;
    let valores = state
        .servicios()?
        .extensiones
        .valores_por_recursos(&empresa_id, &recurso, &recurso_ids)
        .await?;
    ok(json!({ "valores": valores }))
}

/// GET /internal/partes/:id?empresa_id=...
pub async fn parte(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    clave_valida(&headers, state.config.internal_api_key.as_deref())?;
    let Some(empresa_id) = query.get("empresa_id").filter(|valor| !valor.is_empty()) else {
        return Err(ApiError::no_encontrado("Parte", &id));
    };
    let pertenece = state
        .servicios()?
        .partes
        .pertenece_a_empresa(empresa_id, &id)
        .await?;
    if !pertenece {
        return Err(ApiError::no_encontrado("Parte", &id));
    }
    ok(json!({ "id": id, "empresa_id": empresa_id }))
}

fn texto(dto: &Value, campo: &str) -> Result<String, ApiError> {
    dto.get(campo)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            ApiError::validacion(json!([
                {"campo": campo, "restricciones": ["isString"]}
            ]))
        })
}

fn arreglo_texto(dto: &Value, campo: &str) -> Result<Vec<String>, ApiError> {
    let Some(valor) = dto.get(campo) else {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isArray"]}
        ])));
    };
    let Some(lista) = valor.as_array() else {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isArray"]}
        ])));
    };
    if lista.is_empty() {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["arrayNotEmpty"]}
        ])));
    }
    lista
        .iter()
        .map(|item| {
            item.as_str().map(str::to_string).ok_or_else(|| {
                ApiError::validacion(json!([
                    {"campo": campo, "restricciones": ["isString"]}
                ]))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_exigir_clave_interna_configurada() {
        let mut headers = HeaderMap::new();
        headers.insert("x-internal-key", "secreta".parse().expect("header"));
        assert!(clave_valida(&headers, Some("secreta")).is_ok());
        assert!(clave_valida(&headers, Some("otra")).is_err());
        assert!(clave_valida(&headers, None).is_err());
    }

    #[test]
    fn debe_validar_arreglos_del_lote() {
        assert_eq!(
            arreglo_texto(&json!({"ids": ["a", "b"]}), "ids").expect("ids"),
            vec!["a".to_string(), "b".to_string()]
        );
        assert!(arreglo_texto(&json!({"ids": []}), "ids").is_err());
        assert!(arreglo_texto(&json!({"ids": "a"}), "ids").is_err());
    }
}
