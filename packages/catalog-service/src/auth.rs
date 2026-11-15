//! Contexto de usuario inyectado por el API Gateway y guardas de acceso.
//!
//! Réplica de `usuarioDesdeHeaders` + `RolesGuard` + `CapabilityGuard`: el
//! servicio confía en las cabeceras `x-user-*` y `x-tenant`; la autorización
//! fina por fila la aplica RLS en PostgreSQL.

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

pub const TENANT_HEADER: &str = "x-tenant";

pub const ROLES_CATALOGO: &[&str] = &["super_admin", "admin", "empresario"];
pub const ROLES_INVENTARIO_ADMIN: &[&str] = &["super_admin", "admin"];

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

    /// Equivalente a `RolesGuard`: exige contexto y rol permitido salvo que el
    /// usuario tenga un rol configurable con `@Permisos` declarados.
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

    /// Tenant activo obligatorio.
    pub fn tenant(&self) -> Result<&str, ApiError> {
        self.tenant_id.as_deref().ok_or_else(|| {
            ApiError::dominio(
                "TENANT_REQUERIDO",
                "El contexto no tiene tenant (x-tenant).",
            )
        })
    }

    pub fn es_administrador_global(&self) -> bool {
        matches!(self.rol.as_str(), "super_admin" | "admin")
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

pub struct CapacidadesFijas(pub Vec<CapacidadActiva>);

#[async_trait]
impl Capacidades for CapacidadesFijas {
    async fn activas(
        &self,
        _empresa_id: &str,
        _requeridas: &[&str],
    ) -> Result<Vec<CapacidadActiva>, ()> {
        Ok(self.0.clone())
    }
}

/// Aplica el orden real de Nest: `RolesGuard` primero y `CapabilityGuard`
/// después. Las capacidades de método se declaran antes que las de clase.
pub async fn autorizar(
    usuario: &UsuarioContexto,
    roles: &[&str],
    permisos: &[&str],
    capacidades_extra: &[&str],
    capacidades: &dyn Capacidades,
) -> Result<(), ApiError> {
    usuario.exigir_roles(roles, permisos)?;
    if capacidades_extra.is_empty() || usuario.es_administrador_global() {
        return Ok(());
    }
    let tenant = usuario.tenant_id.as_deref().ok_or_else(|| {
        ApiError::dominio(
            "TENANT_REQUERIDO",
            "La operacion requiere un tenant activo.",
        )
    })?;

    let filas = capacidades
        .activas(tenant, capacidades_extra)
        .await
        .map_err(|_| ApiError::servicio_no_disponible("identity-service"))?;
    let activas = filas
        .iter()
        .filter(|fila| fila.activo && fila.empresa_id == tenant)
        .map(|fila| fila.capacidad_codigo.as_str())
        .collect::<std::collections::HashSet<_>>();
    let faltantes = capacidades_extra
        .iter()
        .filter(|capacidad| !activas.contains(**capacidad))
        .map(|capacidad| capacidad.to_string())
        .collect::<Vec<_>>();
    if !faltantes.is_empty() {
        return Err(ApiError::capacidad_no_activa(&faltantes, tenant));
    }
    Ok(())
}

/// `ClaveInternaGuard`: la superficie interna exige la clave compartida.
pub fn clave_interna_valida(headers: &HeaderMap, esperada: Option<&str>) -> bool {
    let recibida = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok());
    match (recibida, esperada) {
        (Some(recibida), Some(esperada)) => recibida == esperada,
        _ => false,
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
    fn debe_permitir_el_rol_configurable_solo_con_permisos_declarados() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "comprador".into(),
            rol_empresa_id: Some("r-1".into()),
            ..Default::default()
        };
        assert!(usuario
            .exigir_roles(ROLES_CATALOGO, &["catalogo.escribir"])
            .is_ok());
        assert!(usuario.exigir_roles(ROLES_CATALOGO, &[]).is_err());
    }

    #[tokio::test]
    async fn debe_bloquear_si_falta_la_capacidad_de_catalogo() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "empresario".into(),
            tenant_id: Some("t-1".into()),
            ..Default::default()
        };
        let capacidades = CapacidadesFijas(vec![CapacidadActiva {
            empresa_id: "t-1".into(),
            capacidad_codigo: "inventario_stock".into(),
            activo: true,
        }]);
        let error = autorizar(
            &usuario,
            ROLES_CATALOGO,
            &[],
            &["catalogo_items"],
            &capacidades,
        )
        .await
        .expect_err("capacidad");
        assert_eq!(error.codigo, "CAPACIDAD_NO_ACTIVA");
        assert_eq!(error.status, axum::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn debe_permitir_cuando_estan_las_dos_capacidades() {
        let usuario = UsuarioContexto {
            user_id: "u-1".into(),
            rol: "empresario".into(),
            tenant_id: Some("t-1".into()),
            ..Default::default()
        };
        let capacidades = CapacidadesFijas(vec![
            CapacidadActiva {
                empresa_id: "t-1".into(),
                capacidad_codigo: "catalogo_items".into(),
                activo: true,
            },
            CapacidadActiva {
                empresa_id: "t-1".into(),
                capacidad_codigo: "inventario_stock".into(),
                activo: true,
            },
        ]);
        assert!(autorizar(
            &usuario,
            ROLES_CATALOGO,
            &[],
            &["catalogo_items", "inventario_stock"],
            &capacidades
        )
        .await
        .is_ok());
    }

    #[test]
    fn debe_comparar_la_clave_interna() {
        let map = headers(&[("x-internal-key", "secreta")]);
        assert!(clave_interna_valida(&map, Some("secreta")));
        assert!(!clave_interna_valida(&map, Some("otra")));
        assert!(!clave_interna_valida(&map, None));
    }
}
