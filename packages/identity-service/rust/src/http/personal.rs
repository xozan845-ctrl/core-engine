//! Handlers de personal e incorporaciones.

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::UsuarioContexto;
use crate::dominio::personal::{
    AceptarInvitacion, AsignarPersonal, CrearInvitacion, CrearPuesto, CrearRol,
};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;

const ROL: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo(
        "descripcion",
        false,
        &[Regla::Texto, Regla::MaxLongitud(500)],
    ),
    campo("permisos", true, &[Regla::Objeto]),
];

const PUESTO: &[ReglaCampo] = &[
    campo("codigo", true, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo("tipo", false, &[Regla::Texto, Regla::Coincide("segmento")]),
    campo("tienda_id", false, &[Regla::Uuid]),
    campo("metadata", false, &[Regla::Objeto]),
];

const ASIGNACION: &[ReglaCampo] = &[
    campo("usuario_id", true, &[Regla::Uuid]),
    campo("rol_empresa_id", true, &[Regla::Uuid]),
    campo("tienda_id", false, &[Regla::Uuid]),
    campo("puesto_id", false, &[Regla::Uuid]),
    campo("metadata", false, &[Regla::Objeto]),
];

const INVITACION: &[ReglaCampo] = &[
    campo("correo", true, &[Regla::Correo]),
    campo("nombre", false, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo("tipo", false, &[Regla::En(&["invitacion", "contratacion"])]),
    campo("rol_empresa_id", true, &[Regla::Uuid]),
    campo("tienda_id", false, &[Regla::Uuid]),
    campo("puesto_id", false, &[Regla::Uuid]),
    campo("metadata", false, &[Regla::Objeto]),
];

const ROL_ESTADO: &[ReglaCampo] = &[campo("activo", true, &[Regla::Booleano])];

const ACEPTAR: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo("contrasena", true, &[Regla::Texto, Regla::MinLongitud(8)]),
];

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn actor(headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    let actor = UsuarioContexto::desde_headers_o_error(headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"], &[])?;
    Ok(actor)
}

pub async fn crear_rol(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(ROL, &dto)?;
    let actor = actor(&headers)?;
    let rol = state
        .servicios()?
        .personal
        .crear_rol(
            &actor,
            &empresa_id,
            CrearRol {
                codigo: texto(&dto, "codigo"),
                nombre: texto(&dto, "nombre"),
                descripcion: dto
                    .get("descripcion")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                permisos: dto
                    .get("permisos")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({})),
            },
        )
        .await?;
    created(rol)
}

pub async fn listar_roles(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let roles = state
        .servicios()?
        .personal
        .listar_roles(&actor, &empresa_id)
        .await?;
    ok(Value::Array(roles))
}

pub async fn actualizar_rol(
    State(state): State<AppState>,
    Path((empresa_id, rol_id)): Path<(String, String)>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(ROL_ESTADO, &dto)?;
    let actor = actor(&headers)?;
    let rol = state
        .servicios()?
        .personal
        .actualizar_rol(
            &actor,
            &empresa_id,
            &rol_id,
            dto.get("activo").and_then(Value::as_bool).unwrap_or(false),
        )
        .await?;
    ok(rol)
}

pub async fn crear_puesto(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(PUESTO, &dto)?;
    let actor = actor(&headers)?;
    let puesto = state
        .servicios()?
        .personal
        .crear_puesto(
            &actor,
            &empresa_id,
            CrearPuesto {
                codigo: texto(&dto, "codigo"),
                nombre: texto(&dto, "nombre"),
                tipo: dto.get("tipo").and_then(Value::as_str).map(str::to_string),
                tienda_id: dto
                    .get("tienda_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                metadata: dto.get("metadata").cloned(),
            },
        )
        .await?;
    created(puesto)
}

pub async fn listar_puestos(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let puestos = state
        .servicios()?
        .personal
        .listar_puestos(&actor, &empresa_id)
        .await?;
    ok(Value::Array(puestos))
}

pub async fn asignar(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(ASIGNACION, &dto)?;
    let actor = actor(&headers)?;
    let asignacion = state
        .servicios()?
        .personal
        .asignar(
            &actor,
            &empresa_id,
            AsignarPersonal {
                usuario_id: texto(&dto, "usuario_id"),
                rol_empresa_id: texto(&dto, "rol_empresa_id"),
                tienda_id: dto
                    .get("tienda_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                puesto_id: dto
                    .get("puesto_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                metadata: dto.get("metadata").cloned(),
            },
        )
        .await?;
    created(asignacion)
}

pub async fn listar_asignaciones(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let asignaciones = state
        .servicios()?
        .personal
        .listar_asignaciones(&actor, &empresa_id)
        .await?;
    ok(Value::Array(asignaciones))
}

pub async fn crear_invitacion(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(INVITACION, &dto)?;
    let actor = actor(&headers)?;
    let invitacion = state
        .servicios()?
        .personal
        .crear_invitacion(
            &actor,
            &empresa_id,
            CrearInvitacion {
                correo: texto(&dto, "correo"),
                nombre: dto
                    .get("nombre")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                tipo: dto.get("tipo").and_then(Value::as_str).map(str::to_string),
                rol_empresa_id: texto(&dto, "rol_empresa_id"),
                tienda_id: dto
                    .get("tienda_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                puesto_id: dto
                    .get("puesto_id")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                metadata: dto.get("metadata").cloned(),
            },
        )
        .await?;
    created(invitacion)
}

pub async fn listar_invitaciones(
    State(state): State<AppState>,
    Path(empresa_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let invitaciones = state
        .servicios()?
        .personal
        .listar_invitaciones(&actor, &empresa_id)
        .await?;
    ok(Value::Array(invitaciones))
}

pub async fn revocar_invitacion(
    State(state): State<AppState>,
    Path((empresa_id, invitacion_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let actor = actor(&headers)?;
    let respuesta = state
        .servicios()?
        .personal
        .revocar_invitacion(&actor, &empresa_id, &invitacion_id)
        .await?;
    ok(respuesta)
}

/// GET /api/v2/incorporaciones/:token
pub async fn consultar_invitacion(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<Response, ApiError> {
    let invitacion = state
        .servicios()?
        .personal
        .consultar_invitacion(&token)
        .await?;
    ok(invitacion)
}

/// POST /api/v2/incorporaciones/:token/aceptar
pub async fn aceptar_invitacion(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(ACEPTAR, &dto)?;
    let sesion = state
        .servicios()?
        .personal
        .aceptar_invitacion(
            &token,
            AceptarInvitacion {
                nombre: texto(&dto, "nombre"),
                contrasena: texto(&dto, "contrasena"),
            },
        )
        .await?;
    created(sesion)
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
    fn debe_validar_los_dtos_de_personal() {
        assert!(validar(
            ROL,
            &json!({"codigo": "estilista", "nombre": "Estilista", "permisos": {"acciones": ["catalogo.leer"]}})
        )
        .is_ok());
        assert!(validar(ROL, &json!({"codigo": "estilista", "nombre": "E"})).is_err());
        assert!(validar(PUESTO, &json!({"codigo": "sillon_1", "nombre": "Sillón 1"})).is_ok());
        assert!(validar(ASIGNACION, &json!({"usuario_id": "no-uuid"})).is_err());
        assert!(validar(
            INVITACION,
            &json!({"correo": "a@b.test", "rol_empresa_id": "11111111-1111-4111-8111-111111111111", "tipo": "otro"})
        )
        .is_err());
        assert!(validar(
            ACEPTAR,
            &json!({"nombre": "Ana", "contrasena": "secreto123"})
        )
        .is_ok());
    }
}
