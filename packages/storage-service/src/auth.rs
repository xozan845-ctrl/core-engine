//! Contexto de usuario inyectado por el API Gateway.
//!
//! El servicio confía en las cabeceras `x-user-*` y `x-tenant`; la autorización
//! fina por fila la aplica RLS en PostgreSQL. `requerir_tenant` replica el
//! contrato de `tenant.ts` (bucket por organización, ADR-24).

use crate::errors::ApiError;
use axum::http::{HeaderMap, HeaderValue};

pub const ROLES: [&str; 9] = [
    "super_admin",
    "admin",
    "empresario",
    "vendedor",
    "comprador",
    "logistica",
    "coordinador",
    "supervisor",
    "operativo",
];

pub const TENANT_HEADER: &str = "x-tenant";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
                .map(|valor| valor.trim().to_string())
                .filter(|valor| !valor.is_empty())
        };
        let user_id = leer("x-user-id")?;
        let rol = leer("x-user-rol")?;
        if !ROLES.contains(&rol.as_str()) {
            return None;
        }
        Some(Self {
            user_id,
            email: leer("x-user-email").unwrap_or_default(),
            rol,
            nombre: leer("x-user-nombre"),
            tenant_id: leer(TENANT_HEADER),
            personal_id: leer("x-user-personal"),
            participacion_porcentaje: leer("x-user-participacion")
                .and_then(|valor| valor.parse().ok()),
            rol_empresa_id: leer("x-user-rol-empresa"),
        })
    }

    pub fn desde_headers_o_error(headers: &HeaderMap) -> Result<Self, ApiError> {
        Self::desde_headers(headers)
            .ok_or_else(|| ApiError::no_autorizado("Se requiere un token valido."))
    }

    pub fn exigir_roles(&self, roles: &[&str]) -> Result<(), ApiError> {
        if roles.contains(&self.rol.as_str()) || self.rol_empresa_id.is_some() {
            return Ok(());
        }
        Err(ApiError::acceso_denegado(format!(
            "Rol \"{}\" no tiene acceso a este recurso.",
            self.rol
        )))
    }

    pub fn tenant(&self) -> Result<&str, ApiError> {
        self.tenant_id
            .as_deref()
            .ok_or_else(|| ApiError::dominio("TENANT_REQUERIDO", "Se requiere una empresa activa."))
    }
}

/// Tenant activo obligatorio. Los roles globales pueden usar el tenant de
/// plataforma configurado explícitamente; nunca se inventa un bucket.
pub fn requerir_tenant(
    usuario: &UsuarioContexto,
    platform_tenant_id: Option<&str>,
) -> Result<String, ApiError> {
    if let Some(tenant) = &usuario.tenant_id {
        return Ok(tenant.clone());
    }
    if matches!(usuario.rol.as_str(), "super_admin" | "admin") {
        if let Some(tenant) = platform_tenant_id.filter(|valor| es_uuid(valor)) {
            return Ok(tenant.to_string());
        }
    }
    Err(ApiError::dominio(
        "TENANT_REQUERIDO",
        "El almacenamiento requiere un tenant (cabecera x-tenant del token).",
    ))
}

/// Guard de clave interna (servicio→servicio): `x-internal-key` debe coincidir.
pub fn exigir_clave_interna(headers: &HeaderMap, clave: &str) -> Result<(), ApiError> {
    let recibida = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok());
    if clave.is_empty() || recibida != Some(clave) {
        return Err(ApiError::acceso_denegado("Clave interna invalida."));
    }
    Ok(())
}

pub fn header_uuid(headers: &HeaderMap, clave: &str) -> Option<String> {
    headers
        .get(clave)
        .and_then(|valor| valor.to_str().ok())
        .map(|valor| valor.trim().to_string())
        .filter(|valor| es_uuid(valor))
}

pub fn es_uuid(valor: &str) -> bool {
    uuid::Uuid::parse_str(valor).is_ok()
}

pub fn header_value(valor: &str) -> Option<HeaderValue> {
    HeaderValue::from_str(valor).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pares: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (clave, valor) in pares {
            map.insert(*clave, HeaderValue::from_static(valor));
        }
        map
    }

    #[test]
    fn debe_leer_el_contexto_inyectado() {
        let usuario = UsuarioContexto::desde_headers(&headers(&[
            ("x-user-id", "u-1"),
            ("x-user-rol", "empresario"),
            ("x-tenant", "11111111-1111-4111-8111-111111111111"),
        ]))
        .expect("usuario");
        assert_eq!(usuario.rol, "empresario");
        assert_eq!(usuario.tenant(), Ok("11111111-1111-4111-8111-111111111111"));
    }

    #[test]
    fn debe_rechazar_roles_desconocidos() {
        assert!(UsuarioContexto::desde_headers(&headers(&[
            ("x-user-id", "u-1"),
            ("x-user-rol", "hacker"),
        ]))
        .is_none());
    }

    #[test]
    fn debe_exigir_tenant_salvo_admin_con_tenant_de_plataforma() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "admin".into(),
            ..Default::default()
        };
        assert_eq!(
            requerir_tenant(&usuario, Some("22222222-2222-4222-8222-222222222222")),
            Ok("22222222-2222-4222-8222-222222222222".into())
        );
        assert!(requerir_tenant(&usuario, None).is_err());
    }

    #[test]
    fn debe_validar_la_clave_interna() {
        assert!(exigir_clave_interna(&headers(&[("x-internal-key", "k")]), "k").is_ok());
        assert!(exigir_clave_interna(&headers(&[("x-internal-key", "otra")]), "k").is_err());
        assert!(exigir_clave_interna(&HeaderMap::new(), "").is_err());
    }

    #[test]
    fn debe_reconocer_uuid() {
        assert!(es_uuid("11111111-1111-4111-8111-111111111111"));
        assert!(!es_uuid("no-uuid"));
    }
}
