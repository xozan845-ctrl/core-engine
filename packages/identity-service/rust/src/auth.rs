//! Primitivas de autenticación: JWT HS256 compatible con Supabase, bcrypt y
//! contexto de usuario inyectado por el gateway.
//!
//! Réplica de `packages/shared/src/auth/jwt.utils.ts` y del contrato de
//! `identity.sesiones` (R-GW-4): el refresh lleva `jti` para poder revocarlo.

use crate::errors::ApiError;
use axum::http::HeaderMap;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

pub const ROLES: [&str; 9] = [
    "super_admin",
    "admin",
    "empresario",
    "vendedor",
    "comprador",
    "coordinador",
    "supervisor",
    "operativo",
    "personal",
];

pub const BCRYPT_COST: u32 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenPayload {
    pub sub: String,
    pub email: String,
    pub rol: String,
    pub tipo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nombre: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participacion_porcentaje: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rol_empresa_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
    #[serde(default)]
    pub iat: i64,
    #[serde(default)]
    pub exp: i64,
}

/// Contexto mínimo del usuario autenticado para emitir tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsuarioToken {
    pub id: String,
    pub email: String,
    pub rol: String,
    pub nombre: String,
    pub tenant_id: Option<String>,
    pub personal_id: Option<String>,
    pub participacion_porcentaje: Option<i64>,
    pub rol_empresa_id: Option<String>,
}

impl UsuarioToken {
    pub fn nuevo(id: impl Into<String>, email: impl Into<String>, rol: impl Into<String>) -> Self {
        let email = email.into();
        Self {
            id: id.into(),
            nombre: email.clone(),
            email,
            rol: rol.into(),
            tenant_id: None,
            personal_id: None,
            participacion_porcentaje: None,
            rol_empresa_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UsuarioSesion {
    pub id: String,
    pub nombre: String,
    pub correo: String,
    pub rol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personal_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participacion_porcentaje: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rol_empresa_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_objeto_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Sesion {
    pub access_token: String,
    pub refresh_token: String,
    pub expira_en: u64,
    pub usuario: UsuarioSesion,
}

fn base_payload(usuario: &UsuarioToken, tipo: &str, ttl_segundos: u64) -> TokenPayload {
    let ahora = chrono::Utc::now().timestamp();
    TokenPayload {
        sub: usuario.id.clone(),
        email: usuario.email.clone(),
        rol: usuario.rol.clone(),
        tipo: tipo.to_string(),
        nombre: Some(usuario.nombre.clone()),
        tenant_id: usuario.tenant_id.clone(),
        personal_id: usuario.personal_id.clone(),
        participacion_porcentaje: usuario.participacion_porcentaje,
        rol_empresa_id: usuario.rol_empresa_id.clone(),
        jti: None,
        iat: ahora,
        exp: ahora + ttl_segundos as i64,
    }
}

pub fn crear_access_token(
    usuario: &UsuarioToken,
    secreto: &str,
    ttl_segundos: u64,
) -> Result<String, ApiError> {
    let payload = base_payload(usuario, "access", ttl_segundos);
    encode(
        &Header::new(Algorithm::HS256),
        &payload,
        &EncodingKey::from_secret(secreto.as_bytes()),
    )
    .map_err(|_| ApiError::interno())
}

pub fn crear_refresh_token(
    usuario: &UsuarioToken,
    secreto: &str,
    ttl_segundos: u64,
    jti: &str,
) -> Result<String, ApiError> {
    let mut payload = base_payload(usuario, "refresh", ttl_segundos);
    payload.jti = Some(jti.to_string());
    encode(
        &Header::new(Algorithm::HS256),
        &payload,
        &EncodingKey::from_secret(secreto.as_bytes()),
    )
    .map_err(|_| ApiError::interno())
}

pub fn verificar_token(token: &str, secreto: &str) -> Result<TokenPayload, ApiError> {
    let mut validacion = Validation::new(Algorithm::HS256);
    validacion.validate_exp = true;
    decode::<TokenPayload>(
        token,
        &DecodingKey::from_secret(secreto.as_bytes()),
        &validacion,
    )
    .map(|datos| datos.claims)
    .map_err(|_| ApiError::no_autorizado("El token es invalido o expiro."))
}

/// Crea una sesión completa. `expira_en` es el TTL del access token en
/// segundos (el contrato TS devuelve 900, no un timestamp).
pub fn crear_sesion(
    usuario: &UsuarioToken,
    secreto: &str,
    access_ttl: u64,
    refresh_ttl: u64,
    jti: &str,
) -> Result<Sesion, ApiError> {
    Ok(Sesion {
        access_token: crear_access_token(usuario, secreto, access_ttl)?,
        refresh_token: crear_refresh_token(usuario, secreto, refresh_ttl, jti)?,
        expira_en: access_ttl,
        usuario: UsuarioSesion {
            id: usuario.id.clone(),
            nombre: usuario.nombre.clone(),
            correo: usuario.email.clone(),
            rol: usuario.rol.clone(),
            tenant_id: usuario.tenant_id.clone(),
            personal_id: usuario.personal_id.clone(),
            participacion_porcentaje: usuario.participacion_porcentaje,
            rol_empresa_id: usuario.rol_empresa_id.clone(),
            avatar_objeto_id: None,
            avatar_url: None,
        },
    })
}

pub fn hash_contrasena(contrasena: &str) -> Result<String, ApiError> {
    bcrypt::hash(contrasena, BCRYPT_COST).map_err(|_| ApiError::interno())
}

pub fn verificar_contrasena(contrasena: &str, hash: &str) -> bool {
    bcrypt::verify(contrasena, hash).unwrap_or(false)
}

/// Contexto que el gateway inyecta en `x-user-*` (R-GW-3).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UsuarioContexto {
    pub user_id: String,
    pub email: String,
    pub rol: String,
    pub nombre: Option<String>,
    pub tenant_id: Option<String>,
    pub personal_id: Option<String>,
    pub participacion_porcentaje: Option<i64>,
    pub rol_empresa_id: Option<String>,
}

impl UsuarioContexto {
    pub fn desde_headers(headers: &HeaderMap) -> Option<Self> {
        let leer = |clave: &str| {
            headers
                .get(clave)
                .and_then(|valor| valor.to_str().ok())
                .map(str::trim)
                .filter(|valor| !valor.is_empty())
                .map(str::to_string)
        };
        let user_id = leer("x-user-id")?;
        let rol = leer("x-user-rol")?;
        Some(Self {
            user_id,
            email: leer("x-user-email").unwrap_or_default(),
            rol,
            nombre: leer("x-user-nombre"),
            tenant_id: leer("x-tenant"),
            personal_id: leer("x-user-personal"),
            participacion_porcentaje: leer("x-user-participacion")
                .and_then(|valor| valor.parse::<i64>().ok()),
            rol_empresa_id: leer("x-user-rol-empresa"),
        })
    }
}

pub fn es_rol(valor: &str) -> bool {
    ROLES.contains(&valor)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRETO: &str = "secreto-de-prueba";

    fn usuario() -> UsuarioToken {
        UsuarioToken {
            id: "11111111-1111-4111-8111-111111111111".into(),
            email: "u@test.dev".into(),
            rol: "empresario".into(),
            nombre: "Usuario".into(),
            tenant_id: Some("22222222-2222-4222-8222-222222222222".into()),
            personal_id: None,
            participacion_porcentaje: Some(40),
            rol_empresa_id: None,
        }
    }

    #[test]
    fn debe_firmar_y_verificar_un_access_token() {
        let token = crear_access_token(&usuario(), SECRETO, 900).expect("token");
        let payload = verificar_token(&token, SECRETO).expect("payload");
        assert_eq!(payload.sub, usuario().id);
        assert_eq!(payload.tipo, "access");
        assert_eq!(
            payload.tenant_id.as_deref(),
            Some("22222222-2222-4222-8222-222222222222")
        );
        assert_eq!(payload.participacion_porcentaje, Some(40));
        assert!(payload.exp > payload.iat);
        assert!(payload.jti.is_none());
    }

    #[test]
    fn debe_incluir_jti_solo_en_el_refresh() {
        let token = crear_refresh_token(&usuario(), SECRETO, 604_800, "sesion-1").expect("token");
        let payload = verificar_token(&token, SECRETO).expect("payload");
        assert_eq!(payload.tipo, "refresh");
        assert_eq!(payload.jti.as_deref(), Some("sesion-1"));
    }

    #[test]
    fn debe_rechazar_un_token_con_otro_secreto() {
        let token = crear_access_token(&usuario(), SECRETO, 900).expect("token");
        let error = verificar_token(&token, "otro").expect_err("invalido");
        assert_eq!(error.codigo, "NO_AUTORIZADO");
    }

    #[test]
    fn debe_hashear_y_verificar_contrasenas() {
        let hash = hash_contrasena("Contrasena123!").expect("hash");
        assert!(hash.starts_with("$2"));
        assert!(verificar_contrasena("Contrasena123!", &hash));
        assert!(!verificar_contrasena("otra", &hash));
    }

    #[test]
    fn debe_leer_el_contexto_del_gateway() {
        let mut headers = HeaderMap::new();
        headers.insert("x-user-id", "u-1".parse().expect("header"));
        headers.insert("x-user-rol", "empresario".parse().expect("header"));
        headers.insert("x-tenant", "t-1".parse().expect("header"));
        headers.insert("x-user-participacion", "25".parse().expect("header"));
        let contexto = UsuarioContexto::desde_headers(&headers).expect("contexto");
        assert_eq!(contexto.user_id, "u-1");
        assert_eq!(contexto.tenant_id.as_deref(), Some("t-1"));
        assert_eq!(contexto.participacion_porcentaje, Some(25));
    }

    #[test]
    fn debe_ignorar_un_contexto_sin_usuario_o_rol() {
        let mut headers = HeaderMap::new();
        headers.insert("x-user-id", "u-1".parse().expect("header"));
        assert!(UsuarioContexto::desde_headers(&headers).is_none());
        assert!(es_rol("empresario"));
        assert!(!es_rol("inventado"));
    }
}
