//! Handlers de `/api/v2/auth` (registro, sesión, contraseña y personal).

use super::validacion::{campo, validar, Regla, ReglaCampo};
use super::Cuerpo;
use crate::auth::{UsuarioContexto, ROLES_LOGISTICA, ROLES_REGISTRABLES};
use crate::dominio::usuarios::{roles_creables, CrearUsuario};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

const REGISTRO: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo("correo", true, &[Regla::Correo]),
    campo("contrasena", true, &[Regla::Texto, Regla::MinLongitud(8)]),
    campo("rol", true, &[Regla::En(&ROLES_REGISTRABLES)]),
];

const LOGIN: &[ReglaCampo] = &[
    campo("correo", true, &[Regla::Correo]),
    campo("contrasena", true, &[Regla::Texto]),
];

const REFRESH: &[ReglaCampo] = &[campo("refresh_token", true, &[Regla::Texto])];

const LOGOUT: &[ReglaCampo] = &[campo("refresh_token", false, &[Regla::Texto])];

const CREAR_USUARIO: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 120)]),
    campo("correo", true, &[Regla::Correo]),
    campo("contrasena", true, &[Regla::Texto, Regla::MinLongitud(8)]),
    campo(
        "rol",
        true,
        &[Regla::En(&[
            "empresario",
            "comprador",
            "admin",
            "logistica",
            "coordinador",
            "supervisor",
            "operativo",
        ])],
    ),
    campo("tenant_id", false, &[Regla::Texto]),
    campo("personal_id", false, &[Regla::Texto]),
];

const CAMBIAR_CONTRASENA: &[ReglaCampo] = &[
    campo("actual", true, &[Regla::Texto, Regla::MinLongitud(1)]),
    campo("nueva", true, &[Regla::Texto, Regla::MinLongitud(8)]),
];

const RESTABLECER: &[ReglaCampo] = &[campo("correo", true, &[Regla::Correo])];

const CONFIRMAR_RESTABLECIMIENTO: &[ReglaCampo] = &[
    campo("token", true, &[Regla::Texto, Regla::Longitud(16, 256)]),
    campo("contrasena", true, &[Regla::Texto, Regla::MinLongitud(8)]),
];

const VINCULAR_PERSONAL: &[ReglaCampo] = &[
    campo(
        "personal_id",
        true,
        &[Regla::Texto, Regla::Longitud(1, 100)],
    ),
    campo("nombre", false, &[Regla::Texto, Regla::Longitud(1, 120)]),
];

fn valor(dto: &Value, campo: &str) -> Option<String> {
    dto.get(campo)
        .and_then(Value::as_str)
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
}

fn created(cuerpo: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(cuerpo)).into_response())
}

fn ok(cuerpo: Value) -> Result<Response, ApiError> {
    Ok(Json(cuerpo).into_response())
}

/// POST /api/v2/auth/registro
pub async fn registrar(
    State(state): State<AppState>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(REGISTRO, &dto)?;
    let datos = CrearUsuario {
        nombre: valor(&dto, "nombre").unwrap_or_default(),
        correo: valor(&dto, "correo").unwrap_or_default(),
        contrasena: valor(&dto, "contrasena").unwrap_or_default(),
        rol: valor(&dto, "rol").unwrap_or_default(),
        tenant_id: valor(&dto, "tenant_id"),
        personal_id: valor(&dto, "personal_id"),
    };
    let sesion = state.servicios()?.auth.registrar(datos).await?;
    created(serde_json::to_value(sesion).map_err(|_| ApiError::interno())?)
}

/// POST /api/v2/auth/login
pub async fn login(
    State(state): State<AppState>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(LOGIN, &dto)?;
    let sesion = state
        .servicios()?
        .auth
        .login(
            &valor(&dto, "correo").unwrap_or_default(),
            &valor(&dto, "contrasena").unwrap_or_default(),
        )
        .await?;
    ok(serde_json::to_value(sesion).map_err(|_| ApiError::interno())?)
}

/// POST /api/v2/auth/refresh
pub async fn refresh(
    State(state): State<AppState>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(REFRESH, &dto)?;
    let token = valor(&dto, "refresh_token")
        .ok_or_else(|| ApiError::dominio("TOKEN_FALTANTE", "El refresh_token es obligatorio."))?;
    let sesion = state.servicios()?.auth.refrescar(&token).await?;
    created(serde_json::to_value(sesion).map_err(|_| ApiError::interno())?)
}

/// POST /api/v2/auth/logout
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(LOGOUT, &dto)?;
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    let token = valor(&dto, "refresh_token");
    state
        .servicios()?
        .auth
        .cerrar_sesion(&usuario.user_id, token.as_deref())
        .await?;
    created(json!({ "ok": true }))
}

/// POST /api/v2/auth/crear-usuario
pub async fn crear_usuario(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CREAR_USUARIO, &dto)?;
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    if !usuario.es_admin_global() {
        return Err(ApiError::acceso_denegado(
            "Solo un administrador puede crear usuarios.",
        ));
    }
    let rol = valor(&dto, "rol").unwrap_or_default();
    let tenant_id = valor(&dto, "tenant_id");
    if ROLES_LOGISTICA.contains(&rol.as_str()) && tenant_id.is_none() {
        return Err(ApiError::dominio(
            "TENANT_REQUERIDO",
            "Los usuarios de logistica requieren tenant_id.",
        ));
    }
    let creado = state
        .servicios()?
        .usuarios
        .crear(CrearUsuario {
            nombre: valor(&dto, "nombre").unwrap_or_default(),
            correo: valor(&dto, "correo").unwrap_or_default(),
            contrasena: valor(&dto, "contrasena").unwrap_or_default(),
            rol,
            tenant_id,
            personal_id: valor(&dto, "personal_id"),
        })
        .await?;
    created(serde_json::to_value(creado).map_err(|_| ApiError::interno())?)
}

/// POST /api/v2/auth/cambiar-contrasena
pub async fn cambiar_contrasena(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CAMBIAR_CONTRASENA, &dto)?;
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    let servicios = state.servicios()?;
    servicios
        .usuarios
        .cambiar_contrasena(
            &usuario.user_id,
            &valor(&dto, "actual").unwrap_or_default(),
            &valor(&dto, "nueva").unwrap_or_default(),
        )
        .await?;
    // Cambiar la contrasena invalida las sesiones abiertas (R-GW-4, ADR-18).
    servicios.auth.cerrar_sesion(&usuario.user_id, None).await?;
    created(json!({ "ok": true }))
}

/// POST /api/v2/auth/restablecer-contrasena
pub async fn restablecer_contrasena(
    State(state): State<AppState>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(RESTABLECER, &dto)?;
    state
        .servicios()?
        .password_reset
        .solicitar(&valor(&dto, "correo").unwrap_or_default())
        .await?;
    created(json!({ "ok": true }))
}

/// POST /api/v2/auth/restablecer-contrasena/confirmar
pub async fn confirmar_restablecimiento(
    State(state): State<AppState>,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(CONFIRMAR_RESTABLECIMIENTO, &dto)?;
    state
        .servicios()?
        .password_reset
        .confirmar(
            &valor(&dto, "token").unwrap_or_default(),
            &valor(&dto, "contrasena").unwrap_or_default(),
        )
        .await?;
    ok(json!({ "ok": true }))
}

/// GET /api/v2/auth/me
pub async fn me(headers: HeaderMap) -> Result<Response, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    ok(serde_json::to_value(usuario).map_err(|_| ApiError::interno())?)
}

/// POST /api/v2/auth/vincular-personal
pub async fn vincular_personal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(dto): Cuerpo,
) -> Result<Response, ApiError> {
    validar(VINCULAR_PERSONAL, &dto)?;
    let usuario = UsuarioContexto::desde_headers_o_error(&headers)?;
    let servicios = state.servicios()?;
    servicios
        .usuarios
        .vincular_personal(
            &usuario.user_id,
            &valor(&dto, "personal_id").unwrap_or_default(),
        )
        .await?;
    if let Some(nombre) = valor(&dto, "nombre") {
        servicios
            .usuarios
            .actualizar_perfil(&usuario.user_id, Some(&nombre), None)
            .await?;
    }
    created(json!({ "ok": true }))
}

/// Roles que el alta administrativa puede asignar (contrato TS).
pub fn roles_del_alta() -> Vec<&'static str> {
    roles_creables().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_el_registro_con_el_shape_de_class_validator() {
        let error = validar(REGISTRO, &json!({})).expect_err("validacion");
        let detalles = error.cuerpo()["detalles"].clone();
        assert_eq!(detalles[0]["campo"], "nombre");
        assert_eq!(detalles[1]["campo"], "correo");
        assert_eq!(detalles[2]["campo"], "contrasena");
        assert_eq!(detalles[3]["campo"], "rol");
        assert!(validar(
            REGISTRO,
            &json!({
                "nombre": "Ana Pérez",
                "correo": "ana@tienda.test",
                "contrasena": "secreto123",
                "rol": "comprador"
            })
        )
        .is_ok());
    }

    #[test]
    fn debe_rechazar_un_rol_no_registrable() {
        let error = validar(
            REGISTRO,
            &json!({
                "nombre": "Ana",
                "correo": "ana@tienda.test",
                "contrasena": "secreto123",
                "rol": "admin"
            }),
        )
        .expect_err("rol");
        assert_eq!(
            error.cuerpo()["detalles"][0]["restricciones"],
            json!(["isIn"])
        );
    }

    #[test]
    fn debe_rechazar_un_token_de_restablecimiento_corto() {
        let error = validar(
            CONFIRMAR_RESTABLECIMIENTO,
            &json!({ "token": "corto", "contrasena": "secreto123" }),
        )
        .expect_err("token");
        assert_eq!(
            error.cuerpo()["detalles"][0]["restricciones"],
            json!(["isLength"])
        );
    }

    #[test]
    fn debe_conservar_los_roles_del_alta() {
        assert_eq!(roles_del_alta().len(), 7);
        assert!(roles_del_alta().contains(&"admin"));
    }
}
