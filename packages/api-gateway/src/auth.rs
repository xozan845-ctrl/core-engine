//! Verificación JWT HS256 y aplicación de la política de acceso en el borde.

use crate::errors::ApiError;
use crate::policy::{Politica, ROLES};
use axum::http::HeaderMap;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use std::collections::HashMap;

pub const TENANT_HEADER: &str = "x-tenant";
pub const PERSONAL_HEADER: &str = "x-user-personal";
pub const PARTICIPACION_HEADER: &str = "x-user-participacion";
pub const ROL_EMPRESA_HEADER: &str = "x-user-rol-empresa";

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct TokenPayload {
    pub sub: String,
    pub email: String,
    pub rol: String,
    pub tipo: String,
    pub nombre: Option<String>,
    pub tenant_id: Option<String>,
    pub personal_id: Option<String>,
    pub participacion_porcentaje: Option<i64>,
    pub rol_empresa_id: Option<String>,
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
    .map_err(|_| ApiError::no_autorizado("Token invalido o expirado."))
}

/// Aplica autenticación y roles. Devuelve las cabeceras de identidad que el
/// gateway debe inyectar al servicio destino.
pub fn aplicar_auth(
    headers: &HeaderMap,
    metodo: &str,
    ruta: &str,
    politica: Option<&Politica>,
    secreto: &str,
) -> Result<HashMap<String, String>, ApiError> {
    let autorizacion = headers
        .get("authorization")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("");
    let tiene_token = autorizacion.starts_with("Bearer ");
    let necesita_token = politica.map(|p| p.roles.is_some()).unwrap_or(true);
    if !tiene_token {
        if necesita_token {
            return Err(ApiError::no_autorizado("Se requiere un token JWT valido."));
        }
        return Ok(HashMap::new());
    }
    let token = &autorizacion[7..];
    let payload = verificar_token(token, secreto)?;
    if payload.tipo != "access" {
        return Err(ApiError::no_autorizado(
            "Token de refresco no es valido para acceso.",
        ));
    }
    if let Some(roles) = politica.and_then(|p| p.roles.as_ref()) {
        if !roles.contains(&payload.rol.as_str()) {
            return Err(ApiError::acceso_denegado(format!(
                "Rol \"{}\" no tiene acceso a este recurso.",
                payload.rol
            )));
        }
    }
    let mut salida = HashMap::new();
    salida.insert("x-user-id".to_string(), payload.sub);
    salida.insert("x-user-email".to_string(), payload.email);
    salida.insert("x-user-rol".to_string(), payload.rol);
    if let Some(nombre) = payload.nombre {
        salida.insert("x-user-nombre".to_string(), nombre);
    }
    if let Some(tenant) = payload.tenant_id {
        salida.insert(TENANT_HEADER.to_string(), tenant);
    }
    if let Some(personal) = payload.personal_id {
        salida.insert(PERSONAL_HEADER.to_string(), personal);
    }
    if let Some(participacion) = payload.participacion_porcentaje {
        salida.insert(PARTICIPACION_HEADER.to_string(), participacion.to_string());
    }
    if let Some(rol_empresa) = payload.rol_empresa_id {
        salida.insert(ROL_EMPRESA_HEADER.to_string(), rol_empresa);
    }
    let _ = (metodo, ruta);
    Ok(salida)
}

pub fn roles_validos() -> &'static [&'static str] {
    &ROLES
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::encontrar_politica;
    use jsonwebtoken::{encode, EncodingKey, Header};

    fn token(secret: &str, rol: &str, tipo: &str) -> String {
        let payload = serde_json::json!({
            "sub": "u-1",
            "email": "u@test.dev",
            "rol": rol,
            "tipo": tipo,
            "tenant_id": "11111111-1111-4111-8111-111111111111",
            "exp": chrono::Utc::now().timestamp() + 3600,
        });
        encode(
            &Header::new(Algorithm::HS256),
            &payload,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .expect("token")
    }

    #[test]
    fn debe_verificar_y_propagar_el_contexto() {
        let token = token("secreto", "empresario", "access");
        let politica = encontrar_politica("/api/v2/catalog/items", "GET").expect("politica");
        let headers = aplicar_auth(
            &HeaderMap::new(),
            "GET",
            "/api/v2/catalog/items",
            Some(politica),
            "secreto",
        )
        .expect_err("sin token");
        assert_eq!(headers.codigo, "NO_AUTORIZADO");

        let mut map = HeaderMap::new();
        map.insert(
            "authorization",
            format!("Bearer {token}").parse().expect("header"),
        );
        let inyectadas = aplicar_auth(
            &map,
            "GET",
            "/api/v2/catalog/items",
            Some(politica),
            "secreto",
        )
        .expect("auth");
        assert_eq!(inyectadas.get("x-user-id").map(String::as_str), Some("u-1"));
        assert_eq!(
            inyectadas.get("x-user-rol").map(String::as_str),
            Some("empresario")
        );
    }

    #[test]
    fn debe_rechazar_rol_insuficiente_y_refresh() {
        let politica = encontrar_politica("/api/v2/finanzas/resumen", "GET").expect("politica");
        let mut map = HeaderMap::new();
        map.insert(
            "authorization",
            format!("Bearer {}", token("secreto", "vendedor", "access"))
                .parse()
                .expect("header"),
        );
        assert_eq!(
            aplicar_auth(
                &map,
                "GET",
                "/api/v2/finanzas/resumen",
                Some(politica),
                "secreto"
            )
            .expect_err("rol")
            .codigo,
            "ACCESO_DENEGADO"
        );

        let mut refresh = HeaderMap::new();
        refresh.insert(
            "authorization",
            format!("Bearer {}", token("secreto", "admin", "refresh"))
                .parse()
                .expect("header"),
        );
        assert!(aplicar_auth(
            &refresh,
            "GET",
            "/api/v2/finanzas/resumen",
            Some(politica),
            "secreto"
        )
        .is_err());
    }
}
