//! Rutas de almacenamiento (públicas, autenticadas, admin e internas).

use crate::auth::{es_uuid, exigir_clave_interna, requerir_tenant, UsuarioContexto};
use crate::errors::ApiError;
use crate::http::Cuerpo;
use crate::state::AppState;
use crate::storage::objetos::{DatosSubida, FiltrosObjetos};
use crate::storage::range::{interpretar_rango, ResultadoRango};
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::Json;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use tokio_util::io::ReaderStream;

const ROLES_ADMIN: [&str; 2] = ["super_admin", "admin"];

fn usuario(headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    UsuarioContexto::desde_headers_o_error(headers)
}

fn tenant(state: &AppState, usuario: &UsuarioContexto) -> Result<String, ApiError> {
    requerir_tenant(usuario, state.config.platform_tenant_id.as_deref())
}

// ── bucket del tenant ───────────────────────────────────────────────────────

pub async fn estado_bucket(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let usuario = usuario(&headers)?;
    let tenant = tenant(&state, &usuario)?;
    let estado = state.buckets()?.asegurar(&tenant).await?;
    Ok(Json(json!(estado)))
}

pub async fn configurar_bucket(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let usuario = usuario(&headers)?;
    usuario.exigir_roles(&ROLES_ADMIN)?;
    let tenant = tenant(&state, &usuario)?;
    let estado = state.buckets()?.asegurar(&tenant).await?;
    let Some(cuota) = cuerpo.get("cuota_bytes").and_then(Value::as_i64) else {
        return Ok((StatusCode::CREATED, Json(json!(estado))));
    };
    let actualizado = state.buckets()?.actualizar_cuota(&tenant, cuota).await?;
    Ok((StatusCode::CREATED, Json(json!(actualizado))))
}

// ── objetos del tenant ──────────────────────────────────────────────────────

pub async fn solicitar_subida(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let usuario = usuario(&headers)?;
    let tenant = tenant(&state, &usuario)?;
    let datos = parsear_subida(&cuerpo)?;
    let solicitud = state
        .objetos()?
        .solicitar_subida(&tenant, Some(&usuario.user_id), datos)
        .await?;
    Ok((StatusCode::CREATED, Json(json!(solicitud))))
}

pub async fn confirmar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let usuario = usuario(&headers)?;
    let tenant = tenant(&state, &usuario)?;
    Ok(Json(json!(state.objetos()?.confirmar(&tenant, &id).await?)))
}

pub async fn listar_objetos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let usuario = usuario(&headers)?;
    let tenant = tenant(&state, &usuario)?;
    let filtros = FiltrosObjetos {
        q: query.get("q").cloned(),
        carpeta: query.get("carpeta").cloned(),
        estado: query.get("estado").cloned(),
        pagina: query.get("pagina").and_then(|valor| valor.parse().ok()),
        limite: query.get("limite").and_then(|valor| valor.parse().ok()),
    };
    Ok(Json(json!(
        state.objetos()?.listar(&tenant, &filtros).await?
    )))
}

pub async fn detalle_objeto(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let usuario = usuario(&headers)?;
    let tenant = tenant(&state, &usuario)?;
    Ok(Json(json!(state.objetos()?.obtener(&tenant, &id).await?)))
}

pub async fn descarga_objeto(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let usuario = usuario(&headers)?;
    let tenant = tenant(&state, &usuario)?;
    Ok(Json(json!(
        state.objetos()?.url_descarga(&tenant, &id).await?
    )))
}

pub async fn eliminar_objeto(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let usuario = usuario(&headers)?;
    let tenant = tenant(&state, &usuario)?;
    state
        .objetos()?
        .eliminar(&tenant, &id, Some(&usuario))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ── media pública (sin auth, ADR-28) ────────────────────────────────────────

pub async fn publico(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    servir_publico(state, id, headers, false).await
}

pub async fn publico_head(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    servir_publico(state, id, headers, true).await
}

async fn servir_publico(
    state: AppState,
    id: String,
    headers: HeaderMap,
    solo_cabeceras: bool,
) -> Result<Response, ApiError> {
    let objetos = state.objetos()?;
    let meta = objetos.obtener_publico_meta(&id).await?;
    let etag = meta.etag.as_ref().map(|valor| {
        if valor.starts_with('"') {
            valor.clone()
        } else {
            format!("\"{valor}\"")
        }
    });
    let rango = headers
        .get(header::RANGE)
        .and_then(|valor| valor.to_str().ok());
    let resultado = interpretar_rango(rango, meta.tamano_bytes);
    let mut response = match resultado {
        ResultadoRango::Invalido => {
            let mut response = Response::new(Body::empty());
            *response.status_mut() = StatusCode::RANGE_NOT_SATISFIABLE;
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!("bytes */{}", meta.tamano_bytes))
                    .unwrap_or_else(|_| HeaderValue::from_static("bytes */0")),
            );
            response
                .headers_mut()
                .insert(header::CONTENT_LENGTH, HeaderValue::from_static("0"));
            return Ok(response);
        }
        ResultadoRango::Completo => {
            let mut response = if solo_cabeceras {
                Response::new(Body::empty())
            } else {
                let objeto = objetos.leer_publico(&id, None).await?;
                let stream = ReaderStream::new(objeto.cuerpo.cuerpo.into_async_read());
                Response::new(Body::from_stream(stream))
            };
            response.headers_mut().insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&meta.tamano_bytes.to_string())
                    .unwrap_or_else(|_| HeaderValue::from_static("0")),
            );
            response
        }
        ResultadoRango::Parcial(rango) => {
            let mut response = if solo_cabeceras {
                Response::new(Body::empty())
            } else {
                let objeto = objetos
                    .leer_publico(&id, Some((rango.inicio, rango.fin)))
                    .await?;
                let stream = ReaderStream::new(objeto.cuerpo.cuerpo.into_async_read());
                Response::new(Body::from_stream(stream))
            };
            *response.status_mut() = StatusCode::PARTIAL_CONTENT;
            response.headers_mut().insert(
                header::CONTENT_RANGE,
                HeaderValue::from_str(&format!(
                    "bytes {}-{}/{}",
                    rango.inicio, rango.fin, meta.tamano_bytes
                ))
                .unwrap_or_else(|_| HeaderValue::from_static("bytes 0-0/0")),
            );
            response.headers_mut().insert(
                header::CONTENT_LENGTH,
                HeaderValue::from_str(&(rango.fin - rango.inicio + 1).to_string())
                    .unwrap_or_else(|_| HeaderValue::from_static("0")),
            );
            response
        }
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&meta.content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("inline"),
    );
    headers.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    if let Some(etag) = etag {
        if let Ok(valor) = HeaderValue::from_str(&etag) {
            headers.insert(header::ETAG, valor);
        }
    }
    Ok(response)
}

// ── admin ───────────────────────────────────────────────────────────────────

pub async fn admin_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let usuario = usuario(&headers)?;
    usuario.exigir_roles(&ROLES_ADMIN)?;
    let pagina = query.get("pagina").and_then(|valor| valor.parse().ok());
    let limite = query.get("limite").and_then(|valor| valor.parse().ok());
    Ok(Json(json!(state.buckets()?.listar(pagina, limite).await?)))
}

pub async fn admin_estado(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let usuario = usuario(&headers)?;
    usuario.exigir_roles(&ROLES_ADMIN)?;
    Ok(Json(json!(state.buckets()?.estado(&tenant_id).await?)))
}

pub async fn admin_actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Value>, ApiError> {
    let usuario = usuario(&headers)?;
    usuario.exigir_roles(&ROLES_ADMIN)?;
    if let Some(cuota) = cuerpo.get("cuota_bytes").and_then(Value::as_i64) {
        state.buckets()?.actualizar_cuota(&tenant_id, cuota).await?;
    }
    if let Some(estado) = cuerpo.get("estado").and_then(Value::as_str) {
        if !matches!(estado, "activo" | "suspendido") {
            return Err(ApiError::validacion(json!([
                {"campo": "estado", "restricciones": ["isIn"]}
            ])));
        }
        state.buckets()?.cambiar_estado(&tenant_id, estado).await?;
    }
    Ok(Json(json!(state.buckets()?.estado(&tenant_id).await?)))
}

pub async fn admin_recalcular(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let usuario = usuario(&headers)?;
    usuario.exigir_roles(&ROLES_ADMIN)?;
    let estado = state.buckets()?.recalcular_uso(&tenant_id).await?;
    Ok((StatusCode::CREATED, Json(json!(estado))))
}

// ── interno ─────────────────────────────────────────────────────────────────

pub async fn interno_estado(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    exigir_clave_interna(
        &headers,
        state.config.internal_api_key.as_deref().unwrap_or(""),
    )?;
    Ok(Json(json!(state.buckets()?.estado(&tenant_id).await?)))
}

pub async fn interno_recalcular(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(tenant_id): Path<String>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    exigir_clave_interna(
        &headers,
        state.config.internal_api_key.as_deref().unwrap_or(""),
    )?;
    let estado = state.buckets()?.recalcular_uso(&tenant_id).await?;
    Ok((StatusCode::CREATED, Json(json!(estado))))
}

pub async fn interno_objeto(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    exigir_clave_interna(
        &headers,
        state.config.internal_api_key.as_deref().unwrap_or(""),
    )?;
    let tenant_id = query.get("tenant_id").ok_or_else(|| {
        ApiError::validacion(json!([
            {"campo": "tenant_id", "restricciones": ["isUuid"]}
        ]))
    })?;
    Ok(Json(json!(state.objetos()?.obtener(tenant_id, &id).await?)))
}

// ── validación de cuerpos ───────────────────────────────────────────────────

fn parsear_subida(cuerpo: &Value) -> Result<DatosSubida, ApiError> {
    let mut errores = Vec::new();
    let objeto = cuerpo.as_object();
    let nombre = objeto
        .and_then(|valor| valor.get("nombre"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| (1..=200).contains(&valor.len()))
        .map(str::to_string);
    if nombre.is_none() {
        errores.push(json!({"campo": "nombre", "restricciones": ["isString", "length"]}));
    }
    let content_type = objeto
        .and_then(|valor| valor.get("content_type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| (3..=120).contains(&valor.len()))
        .map(str::to_string);
    if content_type.is_none() {
        errores.push(json!({"campo": "content_type", "restricciones": ["isString", "length"]}));
    }
    let tamano_bytes = objeto
        .and_then(|valor| valor.get("tamano_bytes"))
        .and_then(Value::as_i64)
        .filter(|valor| *valor > 0);
    if tamano_bytes.is_none() {
        errores.push(json!({"campo": "tamano_bytes", "restricciones": ["isInt", "isPositive"]}));
    }
    let carpeta = objeto
        .and_then(|valor| valor.get("carpeta"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let descripcion = objeto
        .and_then(|valor| valor.get("descripcion"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let etiquetas = objeto
        .and_then(|valor| valor.get("etiquetas"))
        .and_then(Value::as_object)
        .map(|mapa| {
            mapa.iter()
                .take(20)
                .filter_map(|(clave, valor)| {
                    let clave = clave.trim();
                    if clave.is_empty() {
                        return None;
                    }
                    Some((
                        clave.chars().take(40).collect::<String>(),
                        valor
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(200)
                            .collect::<String>(),
                    ))
                })
                .collect::<BTreeMap<_, _>>()
        });
    let publico = objeto
        .and_then(|valor| valor.get("publico"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !errores.is_empty() {
        return Err(ApiError::validacion(Value::Array(errores)));
    }
    Ok(DatosSubida {
        nombre: nombre.expect("validado"),
        content_type: content_type.expect("validado"),
        tamano_bytes: tamano_bytes.expect("validado"),
        carpeta,
        descripcion,
        etiquetas,
        publico,
    })
}

/// Reexport para tests de rutas.
pub fn es_uuid_valido(valor: &str) -> bool {
    es_uuid(valor)
}
