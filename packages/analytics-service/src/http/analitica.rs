//! API de analítica: negocio, empresa, plataforma, cohortes y reportes.

use crate::auth::{exigir_capacidad, UsuarioContexto};
use crate::errors::ApiError;
use crate::reportes::NuevoReporte;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;

const CAPACIDAD: &str = "analitica_negocios";
const ROLES_EMPRESA: [&str; 3] = ["super_admin", "admin", "empresario"];
const ROLES_PLATAFORMA: [&str; 2] = ["super_admin", "admin"];

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/api/v2/analytics/negocios", get(negocios))
        .route("/api/v2/analytics/negocios/cohortes", get(cohortes))
        .route("/api/v2/analytics/empresa", get(empresa))
        .route("/api/v2/analytics/plataforma", get(plataforma))
        .route(
            "/api/v2/analytics/reportes",
            get(listar_reportes).post(crear_reporte),
        )
        .route(
            "/api/v2/analytics/reportes/{id}",
            axum::routing::patch(cambiar_estado),
        )
        .route(
            "/api/v2/analytics/reportes/{id}/ejecutar",
            post(ejecutar_reporte),
        )
        .route(
            "/api/v2/analytics/reportes/{id}/ejecuciones",
            get(ejecuciones_reporte),
        )
}

async fn negocios(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESA).await?;
    let empresa = empresa_de(&usuario, &params)?;
    let negocio = negocio(&usuario, &params);
    let desde = fecha(query(&params, "desde"))?;
    let hasta = fecha(query(&params, "hasta"))?;
    let resumen = crate::analitica::resumen_negocios(
        state.db()?,
        &empresa,
        negocio.as_deref(),
        desde.as_deref(),
        hasta.as_deref(),
    )
    .await?;
    if query(&params, "formato") == Some("csv") {
        return Ok(csv(crate::analitica::resumen_csv(&resumen)));
    }
    Ok(Json(resumen).into_response())
}

async fn cohortes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESA).await?;
    let empresa = empresa_de(&usuario, &params)?;
    let negocio = negocio(&usuario, &params);
    let desde = fecha(query(&params, "desde"))?;
    let hasta = fecha(query(&params, "hasta"))?;
    let resumen = crate::analitica::cohortes_negocios(
        state.db()?,
        &empresa,
        negocio.as_deref(),
        desde.as_deref(),
        hasta.as_deref(),
    )
    .await?;
    if query(&params, "formato") == Some("csv") {
        return Ok(csv(crate::analitica::cohortes_csv(&resumen)));
    }
    Ok(Json(resumen).into_response())
}

async fn empresa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESA).await?;
    let empresa = empresa_de(&usuario, &params)?;
    let desde = fecha(query(&params, "desde"))?;
    let hasta = fecha(query(&params, "hasta"))?;
    let resumen = crate::analitica::resumen_negocios(
        state.db()?,
        &empresa,
        None,
        desde.as_deref(),
        hasta.as_deref(),
    )
    .await?;
    if query(&params, "formato") == Some("csv") {
        return Ok(csv(crate::analitica::resumen_csv(&resumen)));
    }
    Ok(Json(resumen).into_response())
}

async fn plataforma(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let _usuario = autorizar(&state, &headers, &ROLES_PLATAFORMA).await?;
    let desde = fecha(query(&params, "desde"))?;
    let hasta = fecha(query(&params, "hasta"))?;
    let resumen =
        crate::analitica::resumen_plataforma(state.db()?, desde.as_deref(), hasta.as_deref())
            .await?;
    if query(&params, "formato") == Some("csv") {
        return Ok(csv(crate::analitica::plataforma_csv(&resumen)));
    }
    Ok(Json(resumen).into_response())
}

async fn listar_reportes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESA).await?;
    let empresa = empresa_de(&usuario, &params)?;
    Ok(Json(crate::reportes::listar(state.db()?, &empresa).await?))
}

async fn crear_reporte(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
    Json(dto): Json<Value>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESA).await?;
    let empresa = empresa_de(&usuario, &params)?;
    let nombre = texto(&dto, "nombre");
    let tipo = texto(&dto, "tipo");
    let formato = texto(&dto, "formato");
    let frecuencia = texto(&dto, "frecuencia");
    let destino = texto(&dto, "destino");
    let proxima = fecha(Some(texto(&dto, "proxima_ejecucion").as_str()))?
        .ok_or_else(|| ApiError::dominio("FECHA_REQUERIDA", "Indica proxima_ejecucion."))?;
    if !(2..=120).contains(&nombre.trim().chars().count()) {
        return Err(campo_invalido("nombre"));
    }
    if !["analitica", "cohortes"].contains(&tipo.as_str()) {
        return Err(campo_invalido("tipo"));
    }
    if !["json", "csv"].contains(&formato.as_str()) {
        return Err(campo_invalido("formato"));
    }
    if !["diaria", "semanal", "mensual"].contains(&frecuencia.as_str()) {
        return Err(campo_invalido("frecuencia"));
    }
    if !(3..=200).contains(&destino.trim().chars().count()) {
        return Err(campo_invalido("destino"));
    }
    let negocio_id = usuario.negocio_id.clone().or_else(|| {
        dto.get("negocio_id")
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    let reporte = crate::reportes::crear(
        state.db()?,
        &empresa,
        &usuario.user_id,
        NuevoReporte {
            nombre: nombre.trim().to_string(),
            tipo,
            formato,
            frecuencia,
            destino: destino.trim().to_string(),
            proxima_ejecucion: proxima,
            negocio_id,
        },
    )
    .await?;
    Ok((StatusCode::CREATED, Json(reporte)).into_response())
}

async fn cambiar_estado(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
    Json(dto): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESA).await?;
    let empresa = empresa_de(&usuario, &params)?;
    let Some(activo) = dto.get("activo").and_then(Value::as_bool) else {
        return Err(campo_invalido("activo"));
    };
    Ok(Json(
        crate::reportes::cambiar_estado(state.db()?, &empresa, &id, activo).await?,
    ))
}

async fn ejecutar_reporte(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESA).await?;
    let empresa = empresa_de(&usuario, &params)?;
    Ok(Json(
        crate::reportes::ejecutar(state.db()?, &empresa, &id).await?,
    ))
}

async fn ejecuciones_reporte(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_EMPRESA).await?;
    let empresa = empresa_de(&usuario, &params)?;
    Ok(Json(
        crate::reportes::ejecuciones(state.db()?, &empresa, &id).await?,
    ))
}

async fn autorizar(
    state: &AppState,
    headers: &HeaderMap,
    roles: &[&str],
) -> Result<UsuarioContexto, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(headers)?;
    usuario.exigir_roles(roles, &[])?;
    if !usuario.es_admin_global() {
        exigir_capacidad(state.capacidades.as_ref(), &usuario, &[CAPACIDAD]).await?;
    }
    Ok(usuario)
}

fn empresa_de(
    usuario: &UsuarioContexto,
    params: &HashMap<String, String>,
) -> Result<String, ApiError> {
    if usuario.es_empresario_o_configurable() {
        return usuario.tenant().map(str::to_string);
    }
    query(params, "empresaId")
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indique la empresa a consultar."))
}

fn negocio(usuario: &UsuarioContexto, params: &HashMap<String, String>) -> Option<String> {
    usuario
        .negocio_id
        .clone()
        .or_else(|| query(params, "negocioId").map(ToString::to_string))
}

fn query<'a>(params: &'a HashMap<String, String>, clave: &str) -> Option<&'a str> {
    params
        .get(clave)
        .map(String::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
}

fn fecha(valor: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(valor) = valor.filter(|valor| !valor.is_empty()) else {
        return Ok(None);
    };
    let valido = chrono::NaiveDate::parse_from_str(valor, "%Y-%m-%d").is_ok()
        || chrono::DateTime::parse_from_rfc3339(valor).is_ok();
    if !valido {
        return Err(ApiError::validacion(json!([
            {"campo": "fecha", "restricciones": ["isDate"]}
        ])));
    }
    Ok(Some(valor.to_string()))
}

fn texto(dto: &Value, clave: &str) -> String {
    dto.get(clave)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn campo_invalido(campo: &str) -> ApiError {
    ApiError::validacion(json!([{ "campo": campo, "restricciones": ["isDefined"] }]))
}

fn csv(contenido: String) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
        contenido,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_el_formato_de_fecha() {
        assert!(fecha(Some("2026-01-01")).expect("fecha").is_some());
        assert!(fecha(Some("2026-01-01T06:00:00Z"))
            .expect("fecha")
            .is_some());
        assert!(fecha(Some("no-es-fecha")).is_err());
        assert!(fecha(None).expect("vacio").is_none());
    }
}
