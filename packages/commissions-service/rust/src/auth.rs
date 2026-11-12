//! Contexto de usuario inyectado por el API Gateway.
//!
//! Réplica de `usuarioDesdeHeaders` + `RolesGuard`: el servicio confía en las
//! cabeceras `x-user-*` y `x-tenant`; la autorización fina por fila la aplica
//! RLS en PostgreSQL.

use crate::errors::ApiError;
use async_trait::async_trait;
use axum::http::HeaderMap;
use serde::Deserialize;
use std::time::Duration;

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

/// Roles con acceso a la agenda (equivalente a `ROLES_AGENDA`).
pub const ROLES_AGENDA: [&str; 7] = [
    "super_admin",
    "admin",
    "empresario",
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
    /// Devuelve `None` si falta `x-user-id` o si el rol no es conocido, igual
    /// que `usuarioDesdeHeaders`.
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

    pub fn es_rol_agenda(&self) -> bool {
        ROLES_AGENDA.contains(&self.rol.as_str())
    }

    /// Equivalente a `RolesGuard`: exige contexto y rol permitido, salvo que el
    /// usuario tenga rol configurable con permisos declarados.
    pub fn exigir_roles(&self, roles: &[&str], permisos: &[&str]) -> Result<(), ApiError> {
        if roles.contains(&self.rol.as_str()) {
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

    /// Tenant activo obligatorio; mismo código que el controlador original.
    pub fn tenant(&self) -> Result<&str, ApiError> {
        self.tenant_id
            .as_deref()
            .ok_or_else(|| ApiError::dominio("TENANT_REQUERIDO", "Se requiere una empresa activa."))
    }
}

impl UsuarioContexto {
    pub fn desde_headers_o_error(headers: &HeaderMap) -> Result<Self, ApiError> {
        Self::desde_headers(headers)
            .ok_or_else(|| ApiError::no_autorizado("Se requiere un token valido."))
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CapacidadActiva {
    pub empresa_id: String,
    pub capacidad_codigo: String,
    pub activo: bool,
}

/// Consulta de capacidades a identity (R-AR-5): lectura idempotente por la
/// superficie interna, con timeout y fallo cerrado.
#[async_trait]
pub trait Capacidades: Send + Sync {
    async fn activas(
        &self,
        empresa_id: &str,
        requeridas: &[&str],
    ) -> Result<Vec<CapacidadActiva>, ()>;
}

pub struct CapacidadesHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl CapacidadesHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

#[derive(Deserialize)]
struct RespuestaCapacidades {
    #[serde(default)]
    capacidades: Vec<CapacidadActiva>,
}

#[async_trait]
impl Capacidades for CapacidadesHttp {
    async fn activas(
        &self,
        empresa_id: &str,
        requeridas: &[&str],
    ) -> Result<Vec<CapacidadActiva>, ()> {
        if requeridas.is_empty() {
            return Ok(Vec::new());
        }
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| ())?;
        let respuesta = cliente
            .post(format!("{}/internal/empresas/capacidades", self.url))
            .header("content-type", "application/json")
            .header("x-internal-key", &self.clave)
            .json(&serde_json::json!({
                "empresa_ids": [empresa_id],
                "capacidades": requeridas,
            }))
            .send()
            .await
            .map_err(|_| ())?;
        if !respuesta.status().is_success() {
            return Err(());
        }
        respuesta
            .json::<RespuestaCapacidades>()
            .await
            .map(|cuerpo| cuerpo.capacidades)
            .map_err(|_| ())
    }
}

/// Equivalente a `CapabilityGuard`: los administradores de plataforma omiten
/// la consulta; el resto exige tenant activo y todas las capacidades.
pub async fn exigir_capacidad(
    capacidades: &dyn Capacidades,
    usuario: &UsuarioContexto,
    requeridas: &[&str],
) -> Result<(), ApiError> {
    if matches!(usuario.rol.as_str(), "super_admin" | "admin") {
        return Ok(());
    }
    let Some(tenant) = usuario.tenant_id.as_deref() else {
        return Err(ApiError::dominio(
            "TENANT_REQUERIDO",
            "La operacion requiere un tenant activo.",
        ));
    };
    let filas = capacidades
        .activas(tenant, requeridas)
        .await
        .map_err(|_| ApiError::servicio_no_disponible("identity-service"))?;
    let activas: std::collections::HashSet<&str> = filas
        .iter()
        .filter(|fila| fila.activo && fila.empresa_id == tenant)
        .map(|fila| fila.capacidad_codigo.as_str())
        .collect();
    let faltantes: Vec<&str> = requeridas
        .iter()
        .copied()
        .filter(|capacidad| !activas.contains(capacidad))
        .collect();
    if !faltantes.is_empty() {
        return Err(ApiError::nueva(
            "CAPACIDAD_NO_ACTIVA",
            format!(
                "La empresa no tiene activas las capacidades requeridas: {}.",
                faltantes.join(", ")
            ),
            axum::http::StatusCode::FORBIDDEN,
        )
        .con_detalles(serde_json::json!({
            "empresa_id": tenant,
            "capacidades": faltantes,
        })));
    }
    Ok(())
}

/// Implementación para tests: identity no disponible.
pub struct CapacidadesNoDisponible;

#[async_trait]
impl Capacidades for CapacidadesNoDisponible {
    async fn activas(
        &self,
        _empresa_id: &str,
        _requeridas: &[&str],
    ) -> Result<Vec<CapacidadActiva>, ()> {
        Err(())
    }
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
            ("x-user-rol", "empresario"),
            ("x-tenant", "t-1"),
            ("x-user-rol-empresa", "r-1"),
        ]))
        .expect("contexto");
        assert_eq!(usuario.user_id, "u-1");
        assert_eq!(usuario.rol, "empresario");
        assert_eq!(usuario.tenant_id.as_deref(), Some("t-1"));
        assert!(usuario.es_rol_agenda());
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
    fn debe_denegar_un_rol_fuera_de_la_agenda() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "comprador".into(),
            ..Default::default()
        };
        let error = usuario
            .exigir_roles(&ROLES_AGENDA, &["agenda.leer"])
            .expect_err("403");
        assert_eq!(error.status, axum::http::StatusCode::FORBIDDEN);
        assert_eq!(error.codigo, "ACCESO_DENEGADO");
    }

    #[test]
    fn debe_permitir_un_rol_configurable_con_permisos_declarados() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "comprador".into(),
            rol_empresa_id: Some("r-1".into()),
            ..Default::default()
        };
        assert!(usuario
            .exigir_roles(&ROLES_AGENDA, &["agenda.leer"])
            .is_ok());
    }

    #[test]
    fn debe_exigir_tenant_activo() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "empresario".into(),
            ..Default::default()
        };
        let error = usuario.tenant().expect_err("tenant");
        assert_eq!(error.codigo, "TENANT_REQUERIDO");
    }
}
