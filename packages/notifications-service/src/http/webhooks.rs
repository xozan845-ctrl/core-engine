//! API admin de webhooks salientes (ADR-0045).

use super::envelope_con;
use crate::auth::UsuarioContexto;
use crate::errors::ErrorNotificacion;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

type Falla = (StatusCode, Json<Value>);
type Resultado = Result<Response, Falla>;

fn fallo(status: StatusCode, codigo: &str, mensaje: &str) -> Falla {
    (
        status,
        Json(json!({ "codigo": codigo, "mensaje": mensaje })),
    )
}

fn webhooks(state: &AppState) -> Result<&std::sync::Arc<crate::webhooks::Webhooks>, Falla> {
    state.webhooks().map_err(|status| {
        fallo(
            status,
            "SERVICIO_NO_DISPONIBLE",
            "El repositorio de webhooks no está disponible.",
        )
    })
}

fn actor(headers: &HeaderMap) -> Result<UsuarioContexto, Falla> {
    UsuarioContexto::desde_headers(headers).map_err(|_| {
        fallo(
            StatusCode::UNAUTHORIZED,
            "NO_AUTORIZADO",
            "Falta el contexto del usuario.",
        )
    })
}

fn exigir(actor: &UsuarioContexto, roles: &[&str]) -> Result<(), Falla> {
    actor.exigir_roles(roles).map_err(|_| {
        fallo(
            StatusCode::FORBIDDEN,
            "ACCESO_DENEGADO",
            "No tienes permiso para gestionar webhooks.",
        )
    })
}

fn empresa(actor: &UsuarioContexto, candidata: Option<&str>) -> Result<String, Falla> {
    let empresa = candidata
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .or_else(|| actor.tenant_id.clone())
        .ok_or_else(|| {
            fallo(
                StatusCode::BAD_REQUEST,
                "EMPRESA_REQUERIDA",
                "Indica empresa_id o usa el contexto del tenant.",
            )
        })?;
    Uuid::parse_str(&empresa).map_err(|_| {
        fallo(
            StatusCode::BAD_REQUEST,
            "EMPRESA_INVALIDA",
            "empresa_id debe ser un UUID.",
        )
    })?;
    Ok(empresa)
}

fn id_valido(id: &str, entidad: &str) -> Result<(), Falla> {
    Uuid::parse_str(id).map(|_| ()).map_err(|_| {
        fallo(
            StatusCode::BAD_REQUEST,
            "ID_INVALIDO",
            &format!("El id de {entidad} debe ser un UUID."),
        )
    })
}

fn de_error(error: ErrorNotificacion) -> Falla {
    match error {
        ErrorNotificacion::Evento(mensaje) => {
            fallo(StatusCode::BAD_REQUEST, "WEBHOOK_INVALIDO", &mensaje)
        }
        otro => fallo(
            StatusCode::INTERNAL_SERVER_ERROR,
            "ERROR_INTERNO",
            &otro.to_string(),
        ),
    }
}

fn exito(data: Value, path: &str, method: &str, status: StatusCode) -> Response {
    (status, envelope_con(data, path, method, status.as_u16())).into_response()
}

fn eventos(dto: &Value, por_defecto: bool) -> Result<Option<Vec<String>>, Falla> {
    let Some(valor) = dto.get("eventos") else {
        return Ok(if por_defecto { Some(Vec::new()) } else { None });
    };
    if valor.is_null() {
        return Ok(Some(Vec::new()));
    }
    let arreglo = valor.as_array().ok_or_else(|| {
        fallo(
            StatusCode::BAD_REQUEST,
            "EVENTOS_INVALIDOS",
            "eventos debe ser un arreglo de strings.",
        )
    })?;
    let mut eventos = Vec::with_capacity(arreglo.len());
    for valor in arreglo {
        let evento = valor.as_str().ok_or_else(|| {
            fallo(
                StatusCode::BAD_REQUEST,
                "EVENTOS_INVALIDOS",
                "Cada evento debe ser un string.",
            )
        })?;
        eventos.push(evento.to_string());
    }
    Ok(Some(eventos))
}

/// GET /api/v2/admin/notifications/webhooks
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Resultado {
    let actor = actor(&headers)?;
    exigir(&actor, &["super_admin", "admin", "empresario"])?;
    let empresa = empresa(&actor, query.get("empresa_id").map(String::as_str))?;
    let datos = webhooks(&state)?.listar(&empresa).await.map_err(de_error)?;
    Ok(exito(
        json!({ "webhooks": datos }),
        "/api/v2/admin/notifications/webhooks",
        "GET",
        StatusCode::OK,
    ))
}

/// POST /api/v2/admin/notifications/webhooks
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<Value>,
) -> Resultado {
    let actor = actor(&headers)?;
    exigir(&actor, &["super_admin", "admin", "empresario"])?;
    let empresa = empresa(&actor, dto.get("empresa_id").and_then(Value::as_str))?;
    let url = dto
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .ok_or_else(|| {
            fallo(
                StatusCode::BAD_REQUEST,
                "URL_REQUERIDA",
                "La URL del webhook es obligatoria.",
            )
        })?;
    let descripcion = dto.get("descripcion").and_then(Value::as_str).unwrap_or("");
    let eventos = eventos(&dto, true)?.unwrap_or_default();
    let datos = webhooks(&state)?
        .crear(&empresa, url, descripcion, eventos, &actor.user_id)
        .await
        .map_err(de_error)?;
    Ok(exito(
        datos,
        "/api/v2/admin/notifications/webhooks",
        "POST",
        StatusCode::CREATED,
    ))
}

/// PATCH /api/v2/admin/notifications/webhooks/:id
pub async fn actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(dto): Json<Value>,
) -> Resultado {
    let actor = actor(&headers)?;
    exigir(&actor, &["super_admin", "admin", "empresario"])?;
    id_valido(&id, "webhook")?;
    let empresa = empresa(
        &actor,
        dto.get("empresa_id")
            .and_then(Value::as_str)
            .or(actor.tenant_id.as_deref()),
    )?;
    let url = dto.get("url").and_then(Value::as_str);
    let descripcion = dto.get("descripcion").and_then(Value::as_str);
    let activo = dto.get("activo").and_then(Value::as_bool);
    let eventos = eventos(&dto, false)?;
    let datos = webhooks(&state)?
        .actualizar(&id, &empresa, url, descripcion, eventos, activo)
        .await
        .map_err(de_error)?;
    Ok(exito(
        datos,
        &format!("/api/v2/admin/notifications/webhooks/{id}"),
        "PATCH",
        StatusCode::OK,
    ))
}

/// DELETE /api/v2/admin/notifications/webhooks/:id
pub async fn eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Resultado {
    let actor = actor(&headers)?;
    exigir(&actor, &["super_admin", "admin", "empresario"])?;
    id_valido(&id, "webhook")?;
    let empresa = empresa(&actor, query.get("empresa_id").map(String::as_str))?;
    webhooks(&state)?
        .eliminar(&id, &empresa)
        .await
        .map_err(de_error)?;
    Ok(exito(
        json!({ "id": id, "estado": "eliminado" }),
        &format!("/api/v2/admin/notifications/webhooks/{id}"),
        "DELETE",
        StatusCode::OK,
    ))
}

/// POST /api/v2/admin/notifications/webhooks/:id/rotar-secreto
pub async fn rotar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Resultado {
    let actor = actor(&headers)?;
    exigir(&actor, &["super_admin", "admin", "empresario"])?;
    id_valido(&id, "webhook")?;
    let empresa = empresa(&actor, query.get("empresa_id").map(String::as_str))?;
    let datos = webhooks(&state)?
        .rotar_secreto(&id, &empresa)
        .await
        .map_err(de_error)?;
    Ok(exito(
        datos,
        &format!("/api/v2/admin/notifications/webhooks/{id}/rotar-secreto"),
        "POST",
        StatusCode::OK,
    ))
}

/// GET /api/v2/admin/notifications/webhooks/:id/entregas
pub async fn entregas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Resultado {
    let actor = actor(&headers)?;
    exigir(&actor, &["super_admin", "admin", "empresario"])?;
    id_valido(&id, "webhook")?;
    let empresa = empresa(&actor, query.get("empresa_id").map(String::as_str))?;
    let datos = webhooks(&state)?
        .entregas(&id, &empresa)
        .await
        .map_err(de_error)?;
    Ok(exito(
        json!({ "entregas": datos }),
        &format!("/api/v2/admin/notifications/webhooks/{id}/entregas"),
        "GET",
        StatusCode::OK,
    ))
}

/// POST /api/v2/admin/notifications/webhooks/entregas/:id/reintentar
pub async fn reintentar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Resultado {
    let actor = actor(&headers)?;
    exigir(&actor, &["super_admin", "admin", "empresario"])?;
    id_valido(&id, "entrega")?;
    let empresa = empresa(&actor, query.get("empresa_id").map(String::as_str))?;
    webhooks(&state)?
        .reintentar_entrega(&id, &empresa)
        .await
        .map_err(de_error)?;
    Ok(exito(
        json!({ "id": id, "estado": "reintentado" }),
        &format!("/api/v2/admin/notifications/webhooks/entregas/{id}/reintentar"),
        "POST",
        StatusCode::OK,
    ))
}
