//! Handlers de empresas (`/api/v2/empresas`).

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::dominio::empresas::{CrearEmpresa, CrearVendedorEmpresa};
use crate::dominio::media::BACKGROUND_POSICIONES;
use crate::dominio::tema::PLANTILLAS;
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;

const PERFIL: &[ReglaCampo] = &[
    campo("ruc", false, &[Regla::Texto, Regla::MaxLongitud(30)]),
    campo(
        "razon_social",
        false,
        &[Regla::Texto, Regla::MaxLongitud(160)],
    ),
    campo(
        "nombre_comercial",
        false,
        &[Regla::Texto, Regla::MaxLongitud(160)],
    ),
    campo(
        "descripcion",
        false,
        &[Regla::Texto, Regla::MaxLongitud(1000)],
    ),
    campo("telefono", false, &[Regla::Texto, Regla::MaxLongitud(40)]),
    campo("correo_contacto", false, &[Regla::Correo]),
    campo("sitio_web", false, &[Regla::Texto, Regla::MaxLongitud(200)]),
    campo("direccion", false, &[Regla::Texto, Regla::MaxLongitud(300)]),
    campo("ciudad", false, &[Regla::Texto, Regla::MaxLongitud(80)]),
    campo(
        "departamento",
        false,
        &[Regla::Texto, Regla::MaxLongitud(80)],
    ),
    campo("pais", false, &[Regla::Texto, Regla::MaxLongitud(80)]),
    campo("moneda", false, &[Regla::Texto, Regla::Longitud(3, 3)]),
    campo(
        "zona_horaria",
        false,
        &[Regla::Texto, Regla::MaxLongitud(64)],
    ),
    campo("sector", false, &[Regla::Texto, Regla::MaxLongitud(80)]),
    campo(
        "representante_legal",
        false,
        &[Regla::Texto, Regla::MaxLongitud(160)],
    ),
    campo(
        "horario_atencion",
        false,
        &[Regla::Texto, Regla::MaxLongitud(120)],
    ),
    campo("logo_objeto_id", false, &[Regla::Uuid]),
    campo("background_objeto_id", false, &[Regla::Uuid]),
    campo("background_poster_objeto_id", false, &[Regla::Uuid]),
    campo(
        "background_alt",
        false,
        &[Regla::Texto, Regla::MaxLongitud(160)],
    ),
    campo(
        "background_overlay",
        false,
        &[Regla::Entero, Regla::Min(0.0), Regla::Max(100.0)],
    ),
    campo(
        "background_position",
        false,
        &[Regla::En(&BACKGROUND_POSICIONES)],
    ),
    campo("background_activo", false, &[Regla::Booleano]),
    campo("plantilla", false, &[Regla::En(&PLANTILLAS)]),
    campo("tema", false, &[Regla::Objeto]),
];

const CREAR: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 160)]),
    campo("slug", false, &[Regla::Texto, Regla::Longitud(2, 80)]),
    campo(
        "vertical_id",
        true,
        &[Regla::Texto, Regla::Coincide("segmento")],
    ),
    campo(
        "pack_codigo",
        true,
        &[Regla::Texto, Regla::Coincide("segmento")],
    ),
    campo("pack_version", false, &[Regla::Entero, Regla::Min(1.0)]),
];

const PACK: &[ReglaCampo] = &[
    campo(
        "pack_codigo",
        true,
        &[Regla::Texto, Regla::Coincide("segmento")],
    ),
    campo("pack_version", false, &[Regla::Entero, Regla::Min(1.0)]),
];

const SLUG: &[ReglaCampo] = &[campo("slug", true, &[Regla::Texto, Regla::Longitud(2, 80)])];

const VENDEDOR: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo("correo", true, &[Regla::Correo]),
    campo("contrasena", true, &[Regla::Texto, Regla::MinLongitud(8)]),
    campo(
        "participacion_porcentaje",
        true,
        &[Regla::Entero, Regla::Min(0.0), Regla::Max(100.0)],
    ),
];

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

/// POST /api/v2/empresas
pub async fn crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CREAR, &dto)?;
    validar(PERFIL, &dto)?;
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["empresario"], &[])?;
    let empresa = state
        .servicios()?
        .empresas
        .crear(CrearEmpresa {
            usuario_id: actor.user_id,
            nombre: dto
                .get("nombre")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            slug: dto.get("slug").and_then(Value::as_str).map(str::to_string),
            perfil: dto.clone(),
            vertical_id: Some(
                dto.get("vertical_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
            pack_codigo: Some(
                dto.get("pack_codigo")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ),
            pack_version: dto
                .get("pack_version")
                .and_then(Value::as_i64)
                .and_then(|valor| i32::try_from(valor).ok()),
        })
        .await?;
    created(empresa)
}

/// GET /api/v2/empresas/:id
pub async fn obtener(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario", "vendedor"], &[])?;
    let es_global = actor.es_admin_global();
    let (empresa, redirigir) = state
        .servicios()?
        .empresas
        .resolver_para(&actor.user_id, &id, es_global, None)
        .await?;
    if let Some(location) = redirigir {
        let mut respuesta = (StatusCode::MOVED_PERMANENTLY, Json(empresa)).into_response();
        if let Ok(valor) = HeaderValue::from_str(&location) {
            respuesta.headers_mut().insert(header::LOCATION, valor);
        }
        return Ok(respuesta);
    }
    Ok(Json(empresa).into_response())
}

/// PATCH /api/v2/empresas/:id/perfil
pub async fn actualizar_perfil(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(PERFIL, &dto)?;
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    let empresa = state
        .servicios()?
        .empresas
        .actualizar_perfil(&actor.user_id, &id, &dto, actor.es_admin_global())
        .await?;
    Ok(Json(empresa).into_response())
}

/// PATCH /api/v2/empresas/:id/pack
pub async fn actualizar_pack(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(PACK, &dto)?;
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"], &[])?;
    let empresa = state
        .servicios()?
        .empresas
        .actualizar_pack(
            &id,
            dto.get("pack_codigo")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            dto.get("pack_version")
                .and_then(Value::as_i64)
                .and_then(|valor| i32::try_from(valor).ok()),
        )
        .await?;
    Ok(Json(empresa).into_response())
}

/// PATCH /api/v2/empresas/:id/slug
pub async fn actualizar_slug(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(SLUG, &dto)?;
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["empresario"], &[])?;
    let empresa = state
        .servicios()?
        .empresas
        .actualizar_slug(
            &actor.user_id,
            &id,
            dto.get("slug").and_then(Value::as_str).unwrap_or_default(),
        )
        .await?;
    Ok(Json(empresa).into_response())
}

/// POST /api/v2/empresas/:id/vendedores
pub async fn crear_vendedor(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(VENDEDOR, &dto)?;
    let actor = UsuarioContexto::desde_headers_o_error(&headers)?;
    actor.exigir_roles(&["empresario"], &[])?;
    let vendedor = state
        .servicios()?
        .empresas
        .crear_vendedor(
            &actor.user_id,
            &id,
            CrearVendedorEmpresa {
                nombre: dto
                    .get("nombre")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                correo: dto
                    .get("correo")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                contrasena: dto
                    .get("contrasena")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                participacion_porcentaje: dto
                    .get("participacion_porcentaje")
                    .and_then(Value::as_i64)
                    .and_then(|valor| i32::try_from(valor).ok())
                    .unwrap_or(0),
            },
        )
        .await?;
    created(vendedor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_validar_el_alta_y_el_perfil() {
        assert!(validar(
            CREAR,
            &json!({
                "nombre": "Mi Empresa",
                "vertical_id": "retail",
                "pack_codigo": "pack_retail",
                "slug": "mi-empresa"
            })
        )
        .is_ok());
        assert!(validar(
            CREAR,
            &json!({"nombre": "Mi Empresa", "vertical_id": "retail"})
        )
        .is_err());
        assert!(validar(
            PERFIL,
            &json!({"moneda": "NIO", "correo_contacto": "a@b.test"})
        )
        .is_ok());
        assert!(validar(PERFIL, &json!({"moneda": "N"})).is_err());
        assert!(validar(PERFIL, &json!({"background_overlay": 101})).is_err());
        assert!(validar(PERFIL, &json!({"plantilla": "otra"})).is_err());
    }

    #[test]
    fn debe_validar_el_vendedor_y_el_slug() {
        assert!(validar(
            VENDEDOR,
            &json!({
                "nombre": "Vendedor",
                "correo": "v@test.dev",
                "contrasena": "secreto123",
                "participacion_porcentaje": 40
            })
        )
        .is_ok());
        assert!(validar(VENDEDOR, &json!({"participacion_porcentaje": 101})).is_err());
        assert!(validar(SLUG, &json!({"slug": "mi-empresa"})).is_ok());
        assert!(validar(SLUG, &json!({"slug": "x"})).is_err());
    }
}
