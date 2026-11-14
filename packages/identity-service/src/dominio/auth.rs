//! Registro, login, refresh y logout con JWT de corta duración + refresh.
//!
//! Réplica de `auth/auth.service.ts`: cada sesión emitida se persiste en
//! `identity.sesiones` por el `jti` del refresh, de modo que el logout la
//! revoca (R-GW-4, ADR-18).

use crate::auth::{
    crear_sesion as crear_sesion_jwt, verificar_token, Sesion, UsuarioToken, ROLES_REGISTRABLES,
};
use crate::config::Config;
use crate::dominio::sesiones::Sesiones;
use crate::dominio::usuarios::{CrearUsuario, Usuario, Usuarios};
use crate::errors::ApiError;
use chrono::{Duration, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub struct Auth {
    usuarios: Arc<Usuarios>,
    sesiones: Arc<Sesiones>,
    secreto: String,
    access_ttl: u64,
    refresh_ttl: u64,
}

impl Auth {
    pub fn nueva(usuarios: Arc<Usuarios>, sesiones: Arc<Sesiones>, config: &Config) -> Self {
        Self {
            usuarios,
            sesiones,
            secreto: config.jwt_secret.clone(),
            access_ttl: config.access_ttl_segundos(),
            refresh_ttl: config.refresh_ttl_segundos(),
        }
    }

    pub async fn registrar(&self, datos: CrearUsuario) -> Result<Sesion, ApiError> {
        if !ROLES_REGISTRABLES.contains(&datos.rol.as_str()) {
            return Err(ApiError::dominio(
                "ROL_NO_PERMITIDO",
                "El registro publico solo admite empresario o comprador.",
            ));
        }
        let usuario = self.usuarios.crear(datos).await?;
        self.sesion_de(&usuario).await
    }

    pub async fn login(&self, correo: &str, contrasena: &str) -> Result<Sesion, ApiError> {
        let usuario = self
            .usuarios
            .verificar_credenciales(correo, contrasena)
            .await?;
        self.sesion_de(&usuario).await
    }

    pub async fn refrescar(&self, refresh_token: &str) -> Result<Sesion, ApiError> {
        let payload = verificar_token(refresh_token, &self.secreto)
            .map_err(|_| ApiError::no_autorizado("El refresh_token es invalido o expiro."))?;
        if payload.tipo != "refresh" {
            return Err(ApiError::no_autorizado("El token no es de refresco."));
        }
        let Some(jti) = payload.jti.as_deref() else {
            return Err(ApiError::no_autorizado(
                "El token no tiene una sesion asociada.",
            ));
        };
        let Some(sesion) = self.sesiones.activa_por_jti(jti).await? else {
            return Err(ApiError::no_autorizado(
                "La sesion fue cerrada; inicia sesion de nuevo.",
            ));
        };
        if sesion.usuario_id != payload.sub {
            return Err(ApiError::no_autorizado(
                "La sesion fue cerrada; inicia sesion de nuevo.",
            ));
        }
        let Some(usuario) = self.usuarios.encontrar_por_id(&payload.sub).await? else {
            return Err(ApiError::no_autorizado(
                "El usuario del token ya no existe.",
            ));
        };
        self.emitir(&usuario, jti)
    }

    /// `None` significa "todas las sesiones" (logout global).
    pub async fn cerrar_sesion(
        &self,
        usuario_id: &str,
        refresh_token: Option<&str>,
    ) -> Result<u64, ApiError> {
        let Some(token) = refresh_token.filter(|valor| !valor.trim().is_empty()) else {
            return self.sesiones.revocar_todas(usuario_id).await;
        };
        let Ok(payload) = verificar_token(token, &self.secreto) else {
            return Ok(0);
        };
        if payload.tipo == "refresh"
            && payload.sub == usuario_id
            && payload.jti.as_deref().is_some()
        {
            if let Some(jti) = payload.jti.as_deref() {
                return self.sesiones.revocar(jti, usuario_id).await;
            }
        }
        Ok(0)
    }

    /// Crea una sesión para un usuario ya autenticado (aceptar invitación).
    pub async fn crear_sesion(&self, usuario: &Usuario) -> Result<Sesion, ApiError> {
        self.sesion_de(usuario).await
    }

    async fn sesion_de(&self, usuario: &Usuario) -> Result<Sesion, ApiError> {
        let jti = Uuid::new_v4().to_string();
        let expira_en = Utc::now() + Duration::seconds(self.refresh_ttl as i64);
        self.sesiones.crear(&usuario.id, &jti, expira_en).await?;
        self.emitir(usuario, &jti)
    }

    fn emitir(&self, usuario: &Usuario, jti: &str) -> Result<Sesion, ApiError> {
        let token = a_token(usuario);
        let mut sesion = crear_sesion_jwt(
            &token,
            &self.secreto,
            self.access_ttl,
            self.refresh_ttl,
            jti,
        )?;
        sesion.usuario.avatar_objeto_id = usuario.avatar_objeto_id.clone();
        sesion.usuario.avatar_url = usuario.avatar_url.clone();
        Ok(sesion)
    }
}

fn a_token(usuario: &Usuario) -> UsuarioToken {
    UsuarioToken {
        id: usuario.id.clone(),
        email: usuario.correo.clone(),
        rol: usuario.rol.clone(),
        nombre: usuario.nombre.clone(),
        tenant_id: usuario.tenant_id.clone(),
        personal_id: usuario.personal_id.clone(),
        participacion_porcentaje: usuario.participacion_porcentaje,
        rol_empresa_id: usuario.rol_empresa_id.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_copiar_el_contexto_del_usuario_al_token() {
        let usuario = Usuario {
            id: "u-1".into(),
            nombre: "Ana".into(),
            correo: "ana@test.dev".into(),
            rol: "empresario".into(),
            tenant_id: Some("t-1".into()),
            personal_id: None,
            participacion_porcentaje: Some(50),
            rol_empresa_id: None,
            avatar_objeto_id: Some("a-1".into()),
            avatar_url: Some("/a".into()),
            creado_en: "2026-11-13T00:00:00.000Z".into(),
        };
        let token = a_token(&usuario);
        assert_eq!(token.id, "u-1");
        assert_eq!(token.tenant_id.as_deref(), Some("t-1"));
        assert_eq!(token.participacion_porcentaje, Some(50));
    }
}
