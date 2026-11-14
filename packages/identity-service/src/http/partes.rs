//! Handlers de partes universales: contactos, direcciones y relaciones.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

const PARTE: &[ReglaCampo] = &[
    campo("tipo", true, &[Regla::En(&["persona", "organizacion"])]),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(1, 240)]),
    campo(
        "nombre_comercial",
        false,
        &[Regla::Texto, Regla::MaxLongitud(240)],
    ),
    campo(
        "identificador_tipo",
        false,
        &[Regla::Texto, Regla::MaxLongitud(40)],
    ),
    campo(
        "identificador_valor",
        false,
        &[Regla::Texto, Regla::MaxLongitud(120)],
    ),
    campo(
        "estado",
        false,
        &[Regla::En(&["activo", "inactivo", "archivado"])],
    ),
    campo("metadata", false, &[Regla::Objeto]),
];

const PARTE_PARCIAL: &[ReglaCampo] = &[
    campo("tipo", false, &[Regla::En(&["persona", "organizacion"])]),
    campo("nombre", false, &[Regla::Texto, Regla::Longitud(1, 240)]),
    campo(
        "nombre_comercial",
        false,
        &[Regla::Texto, Regla::MaxLongitud(240)],
    ),
    campo(
        "identificador_tipo",
        false,
        &[Regla::Texto, Regla::MaxLongitud(40)],
    ),
    campo(
        "identificador_valor",
        false,
        &[Regla::Texto, Regla::MaxLongitud(120)],
    ),
    campo(
        "estado",
        false,
        &[Regla::En(&["activo", "inactivo", "archivado"])],
    ),
    campo("metadata", false, &[Regla::Objeto]),
];

const CONTACTO: &[ReglaCampo] = &[
    campo(
        "tipo",
        true,
        &[Regla::En(&["email", "telefono", "whatsapp", "web", "otro"])],
    ),
    campo("valor", true, &[Regla::Texto, Regla::Longitud(1, 240)]),
    campo("etiqueta", false, &[Regla::Texto, Regla::MaxLongitud(120)]),
    campo("es_principal", false, &[Regla::Booleano]),
    campo("metadata", false, &[Regla::Objeto]),
];

const CONTACTO_PARCIAL: &[ReglaCampo] = &[
    campo(
        "tipo",
        false,
        &[Regla::En(&["email", "telefono", "whatsapp", "web", "otro"])],
    ),
    campo("valor", false, &[Regla::Texto, Regla::Longitud(1, 240)]),
    campo("etiqueta", false, &[Regla::Texto, Regla::MaxLongitud(120)]),
    campo("es_principal", false, &[Regla::Booleano]),
    campo("metadata", false, &[Regla::Objeto]),
];

const DIRECCION: &[ReglaCampo] = &[
    campo(
        "tipo",
        true,
        &[Regla::En(&[
            "fiscal",
            "comercial",
            "entrega",
            "operativa",
            "otro",
        ])],
    ),
    campo("etiqueta", false, &[Regla::Texto, Regla::MaxLongitud(120)]),
    campo("linea_1", true, &[Regla::Texto, Regla::Longitud(1, 240)]),
    campo("linea_2", false, &[Regla::Texto, Regla::MaxLongitud(240)]),
    campo("ciudad", false, &[Regla::Texto, Regla::MaxLongitud(120)]),
    campo(
        "departamento",
        false,
        &[Regla::Texto, Regla::MaxLongitud(120)],
    ),
    campo("pais", false, &[Regla::Texto, Regla::MaxLongitud(120)]),
    campo(
        "codigo_postal",
        false,
        &[Regla::Texto, Regla::MaxLongitud(32)],
    ),
    campo(
        "referencias",
        false,
        &[Regla::Texto, Regla::MaxLongitud(500)],
    ),
    campo(
        "latitud",
        false,
        &[Regla::Numero, Regla::Min(-90.0), Regla::Max(90.0)],
    ),
    campo(
        "longitud",
        false,
        &[Regla::Numero, Regla::Min(-180.0), Regla::Max(180.0)],
    ),
    campo("es_principal", false, &[Regla::Booleano]),
    campo("metadata", false, &[Regla::Objeto]),
];

const DIRECCION_PARCIAL: &[ReglaCampo] = &[
    campo(
        "tipo",
        false,
        &[Regla::En(&[
            "fiscal",
            "comercial",
            "entrega",
            "operativa",
            "otro",
        ])],
    ),
    campo("etiqueta", false, &[Regla::Texto, Regla::MaxLongitud(120)]),
    campo("linea_1", false, &[Regla::Texto, Regla::Longitud(1, 240)]),
    campo("linea_2", false, &[Regla::Texto, Regla::MaxLongitud(240)]),
    campo("ciudad", false, &[Regla::Texto, Regla::MaxLongitud(120)]),
    campo(
        "departamento",
        false,
        &[Regla::Texto, Regla::MaxLongitud(120)],
    ),
    campo("pais", false, &[Regla::Texto, Regla::MaxLongitud(120)]),
    campo(
        "codigo_postal",
        false,
        &[Regla::Texto, Regla::MaxLongitud(32)],
    ),
    campo(
        "referencias",
        false,
        &[Regla::Texto, Regla::MaxLongitud(500)],
    ),
    campo(
        "latitud",
        false,
        &[Regla::Numero, Regla::Min(-90.0), Regla::Max(90.0)],
    ),
    campo(
        "longitud",
        false,
        &[Regla::Numero, Regla::Min(-180.0), Regla::Max(180.0)],
    ),
    campo("es_principal", false, &[Regla::Booleano]),
    campo("metadata", false, &[Regla::Objeto]),
];

const RELACION: &[ReglaCampo] = &[
    campo("parte_destino_id", true, &[Regla::Uuid]),
    campo("tipo", true, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("metadata", false, &[Regla::Objeto]),
];

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn actor(headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    UsuarioContexto::desde_headers_o_error(headers)
}

/// GET /api/v2/empresas/:empresaId/partes
pub async fn listar(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let partes = state
        .servicios()?
        .partes
        .listar(
            &empresa_id,
            &actor,
            query.get("q").map(String::as_str),
            query.get("tipo").map(String::as_str),
            query
                .get("limite")
                .and_then(|valor| valor.parse::<i64>().ok()),
        )
        .await?;
    ok(Value::Array(partes))
}

/// POST /api/v2/empresas/:empresaId/partes
pub async fn crear(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(PARTE, &dto)?;
    let actor = actor(&headers)?;
    let parte = state
        .servicios()?
        .partes
        .crear(&empresa_id, &actor, &dto)
        .await?;
    created(parte)
}

/// GET /api/v2/empresas/:empresaId/partes/:parteId
pub async fn obtener(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let parte = state
        .servicios()?
        .partes
        .obtener(&empresa_id, &parte_id, &actor)
        .await?;
    ok(parte)
}

/// PATCH /api/v2/empresas/:empresaId/partes/:parteId
pub async fn actualizar(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(PARTE_PARCIAL, &dto)?;
    let actor = actor(&headers)?;
    let parte = state
        .servicios()?
        .partes
        .actualizar(&empresa_id, &parte_id, &actor, &dto)
        .await?;
    ok(parte)
}

/// DELETE /api/v2/empresas/:empresaId/partes/:parteId
pub async fn eliminar(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let respuesta = state
        .servicios()?
        .partes
        .eliminar(&empresa_id, &parte_id, &actor)
        .await?;
    ok(respuesta)
}

/// GET /api/v2/empresas/:empresaId/partes/:parteId/contactos
pub async fn listar_contactos(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let contactos = state
        .servicios()?
        .partes
        .listar_contactos(&empresa_id, &parte_id, &actor)
        .await?;
    ok(Value::Array(contactos))
}

pub async fn crear_contacto(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CONTACTO, &dto)?;
    let actor = actor(&headers)?;
    let contacto = state
        .servicios()?
        .partes
        .crear_contacto(&empresa_id, &parte_id, &actor, &dto)
        .await?;
    created(contacto)
}

pub async fn actualizar_contacto(
    State(state): State<AppState>,
    Path((empresa_id, parte_id, contacto_id)): Path<(String, String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CONTACTO_PARCIAL, &dto)?;
    let actor = actor(&headers)?;
    let contacto = state
        .servicios()?
        .partes
        .actualizar_contacto(&empresa_id, &parte_id, &contacto_id, &actor, &dto)
        .await?;
    ok(contacto)
}

pub async fn eliminar_contacto(
    State(state): State<AppState>,
    Path((empresa_id, parte_id, contacto_id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let respuesta = state
        .servicios()?
        .partes
        .eliminar_contacto(&empresa_id, &parte_id, &contacto_id, &actor)
        .await?;
    ok(respuesta)
}

/// GET /api/v2/empresas/:empresaId/partes/:parteId/direcciones
pub async fn listar_direcciones(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let direcciones = state
        .servicios()?
        .partes
        .listar_direcciones(&empresa_id, &parte_id, &actor)
        .await?;
    ok(Value::Array(direcciones))
}

pub async fn crear_direccion(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(DIRECCION, &dto)?;
    let actor = actor(&headers)?;
    let direccion = state
        .servicios()?
        .partes
        .crear_direccion(&empresa_id, &parte_id, &actor, &dto)
        .await?;
    created(direccion)
}

pub async fn actualizar_direccion(
    State(state): State<AppState>,
    Path((empresa_id, parte_id, direccion_id)): Path<(String, String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(DIRECCION_PARCIAL, &dto)?;
    let actor = actor(&headers)?;
    let direccion = state
        .servicios()?
        .partes
        .actualizar_direccion(&empresa_id, &parte_id, &direccion_id, &actor, &dto)
        .await?;
    ok(direccion)
}

pub async fn eliminar_direccion(
    State(state): State<AppState>,
    Path((empresa_id, parte_id, direccion_id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let respuesta = state
        .servicios()?
        .partes
        .eliminar_direccion(&empresa_id, &parte_id, &direccion_id, &actor)
        .await?;
    ok(respuesta)
}

/// GET /api/v2/empresas/:empresaId/partes/:parteId/relaciones
pub async fn listar_relaciones(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let relaciones = state
        .servicios()?
        .partes
        .listar_relaciones(&empresa_id, &parte_id, &actor)
        .await?;
    ok(Value::Array(relaciones))
}

pub async fn crear_relacion(
    State(state): State<AppState>,
    Path((empresa_id, parte_id)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(RELACION, &dto)?;
    let actor = actor(&headers)?;
    let relacion = state
        .servicios()?
        .partes
        .crear_relacion(
            &empresa_id,
            &parte_id,
            &actor,
            texto(&dto, "parte_destino_id").as_str(),
            texto(&dto, "tipo").as_str(),
            dto.get("metadata"),
        )
        .await?;
    created(relacion)
}

pub async fn eliminar_relacion(
    State(state): State<AppState>,
    Path((empresa_id, parte_id, relacion_id)): Path<(String, String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let respuesta = state
        .servicios()?
        .partes
        .eliminar_relacion(&empresa_id, &parte_id, &relacion_id, &actor)
        .await?;
    ok(respuesta)
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_validar_los_dtos_de_partes() {
        assert!(validar(PARTE, &json!({"tipo": "persona", "nombre": "Ana"})).is_ok());
        assert!(validar(PARTE, &json!({"tipo": "robot", "nombre": "Ana"})).is_err());
        assert!(validar(CONTACTO, &json!({"tipo": "email", "valor": "a@b.test"})).is_ok());
        assert!(validar(
            DIRECCION,
            &json!({"tipo": "entrega", "linea_1": "Calle 1", "latitud": 91})
        )
        .is_err());
        assert!(validar(
            RELACION,
            &json!({"parte_destino_id": "11111111-1111-4111-8111-111111111111", "tipo": "cliente"})
        )
        .is_ok());
    }
}
