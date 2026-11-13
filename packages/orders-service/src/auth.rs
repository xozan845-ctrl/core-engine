//! Contexto de usuario inyectado por el API Gateway y guardas de autorización.
//!
//! Réplica de `usuarioDesdeHeaders`, `RolesGuard` y `ClaveInternaGuard`: el
//! servicio confía en las cabeceras `x-user-*` y `x-tenant`; la autorización
//! fina por fila la aplica RLS en PostgreSQL.

use crate::errors::ApiError;
use axum::http::HeaderMap;

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
    /// Devuelve `None` si falta `x-user-id` o si el rol no es conocido.
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

    /// Equivalente a `RolesGuard`: exige contexto y rol permitido, salvo que el
    /// usuario tenga rol configurable con permisos declarados.
    pub fn exigir_roles(&self, roles: &[&str], permisos: &[&str]) -> Result<(), ApiError> {
        if roles.is_empty() || roles.contains(&self.rol.as_str()) {
            return Ok(());
        }
        if self.rol_empresa_id.is_some() && !permisos.is_empty() {
            return Ok(());
        }
        Err(ApiError::acceso_denegado(format!(
            "Rol \"{}\" no tiene acceso a este recurso.",
            self.rol
        )))
    }

    pub fn es_admin_global(&self) -> bool {
        self.rol == "super_admin" || self.rol == "admin"
    }

    pub fn es_empresario_o_rol_configurable(&self) -> bool {
        self.rol == "empresario" || self.rol_empresa_id.is_some()
    }

    pub fn tenant(&self) -> Result<&str, ApiError> {
        self.tenant_id
            .as_deref()
            .ok_or_else(|| ApiError::dominio("TENANT_REQUERIDO", "Se requiere una empresa activa."))
    }

    /// Empresa sobre la que opera el actor. El empresario y los roles
    /// configurables solo pueden operar su tenant; el admin global indica
    /// `empresa_id` explícitamente.
    pub fn empresa_de(&self, solicitada: Option<&str>) -> Result<String, ApiError> {
        if self.es_empresario_o_rol_configurable() {
            let tenant = self
                .tenant_id
                .as_deref()
                .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."))?;
            if let Some(pedida) = solicitada.filter(|valor| !valor.is_empty()) {
                if pedida != tenant {
                    return Err(ApiError::dominio(
                        "EMPRESA_NO_AUTORIZADA",
                        "No puedes operar otra empresa.",
                    ));
                }
            }
            return Ok(tenant.to_string());
        }
        solicitada
            .filter(|valor| !valor.is_empty())
            .map(ToString::to_string)
            .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indica empresa_id."))
    }

    /// Ejecuta una lectura interna con el contexto de servicio confiable.
    pub fn servicio_interno() -> Self {
        Self {
            user_id: "00000000-0000-0000-0000-000000000000".to_string(),
            email: String::new(),
            rol: "super_admin".to_string(),
            ..Default::default()
        }
    }
}

/// Equivalente a `ClaveInternaGuard`. La clave configurada es obligatoria.
pub fn exigir_clave_interna(headers: &HeaderMap, esperada: Option<&str>) -> Result<(), ApiError> {
    let recibida = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("");
    let esperada = esperada.unwrap_or("");
    if esperada.is_empty() || recibida != esperada {
        return Err(ApiError::acceso_denegado("Clave interna invalida."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pares: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (clave, valor) in pares {
            map.insert(*clave, HeaderValue::from_static(valor));
        }
        map
    }

    #[test]
    fn debe_leer_el_contexto_inyectado_por_el_gateway() {
        let usuario = UsuarioContexto::desde_headers(&headers(&[
            ("x-user-id", "u-1"),
            ("x-user-email", "ana@test.dev"),
            ("x-user-rol", "comprador"),
            ("x-tenant", "t-1"),
            ("x-user-participacion", "25"),
            ("x-user-rol-empresa", "r-1"),
        ]))
        .expect("contexto");
        assert_eq!(usuario.user_id, "u-1");
        assert_eq!(usuario.rol, "comprador");
        assert_eq!(usuario.tenant_id.as_deref(), Some("t-1"));
        assert_eq!(usuario.participacion_porcentaje, Some(25));
        assert_eq!(usuario.rol_empresa_id.as_deref(), Some("r-1"));
    }

    #[test]
    fn debe_rechazar_roles_desconocidos_o_sin_user_id() {
        assert!(UsuarioContexto::desde_headers(&headers(&[
            ("x-user-id", "u-1"),
            ("x-user-rol", "hacker"),
        ]))
        .is_none());
        assert!(UsuarioContexto::desde_headers(&headers(&[("x-user-rol", "admin")])).is_none());
    }

    #[test]
    fn debe_denegar_un_rol_fuera_del_conjunto() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "comprador".into(),
            ..Default::default()
        };
        let error = usuario
            .exigir_roles(&["vendedor"], &["ordenes.leer"])
            .expect_err("403");
        assert_eq!(error.status, axum::http::StatusCode::FORBIDDEN);
        assert_eq!(error.codigo, "ACCESO_DENEGADO");
    }

    #[test]
    fn debe_permitir_un_rol_configurable_con_permisos_declarados() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "operativo".into(),
            rol_empresa_id: Some("r-1".into()),
            ..Default::default()
        };
        assert!(usuario
            .exigir_roles(&["vendedor"], &["ordenes.leer"])
            .is_ok());
    }

    #[test]
    fn debe_resolver_la_empresa_del_actor() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "empresario".into(),
            tenant_id: Some("e-1".into()),
            ..Default::default()
        };
        assert_eq!(usuario.empresa_de(None).expect("empresa"), "e-1");
        assert_eq!(usuario.empresa_de(Some("e-1")).expect("empresa"), "e-1");
        assert_eq!(
            usuario.empresa_de(Some("e-2")).expect_err("otra").codigo,
            "EMPRESA_NO_AUTORIZADA"
        );
    }

    #[test]
    fn debe_exigir_la_clave_interna_configurada() {
        assert!(
            exigir_clave_interna(&headers(&[("x-internal-key", "secreto")]), Some("secreto"))
                .is_ok()
        );
        assert!(exigir_clave_interna(&headers(&[]), Some("secreto")).is_err());
        assert!(
            exigir_clave_interna(&headers(&[("x-internal-key", "otra")]), Some("secreto")).is_err()
        );
        assert!(exigir_clave_interna(&headers(&[("x-internal-key", "secreto")]), None).is_err());
    }
}
