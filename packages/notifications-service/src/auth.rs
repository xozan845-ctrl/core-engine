//! Contexto mínimo de usuario inyectado por el gateway.

use axum::http::{HeaderMap, StatusCode};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsuarioContexto {
    pub user_id: String,
    pub rol: String,
    pub tenant_id: Option<String>,
}

impl UsuarioContexto {
    pub fn desde_headers(headers: &HeaderMap) -> Result<Self, StatusCode> {
        let valor = |clave: &str| {
            headers
                .get(clave)
                .and_then(|valor| valor.to_str().ok())
                .map(str::trim)
                .filter(|valor| !valor.is_empty())
                .map(str::to_string)
        };
        let user_id = valor("x-user-id").ok_or(StatusCode::UNAUTHORIZED)?;
        let rol = valor("x-user-rol").ok_or(StatusCode::UNAUTHORIZED)?;
        Ok(Self {
            user_id,
            rol,
            tenant_id: valor("x-tenant"),
        })
    }

    pub fn exigir_roles(&self, roles: &[&str]) -> Result<(), StatusCode> {
        if roles.contains(&self.rol.as_str()) {
            Ok(())
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pares: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (clave, valor) in pares {
            map.insert(*clave, valor.parse().expect("header"));
        }
        map
    }

    #[test]
    fn debe_exigir_contexto_y_rol() {
        assert_eq!(
            UsuarioContexto::desde_headers(&HeaderMap::new()),
            Err(StatusCode::UNAUTHORIZED)
        );
        let actor = UsuarioContexto::desde_headers(&headers(&[
            ("x-user-id", "u-1"),
            ("x-user-rol", "admin"),
            ("x-tenant", "t-1"),
        ]))
        .expect("actor");
        assert!(actor.exigir_roles(&["admin"]).is_ok());
        assert_eq!(
            actor.exigir_roles(&["empresario"]),
            Err(StatusCode::FORBIDDEN)
        );
    }
}
