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
use axum::http::StatusCode;
use chrono::{Duration, Utc};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

pub struct Auth {
    usuarios: Arc<Usuarios>,
    sesiones: Arc<Sesiones>,
    secreto: String,
    access_ttl: u64,
    refresh_ttl: u64,
    exigir_correo_verificado: bool,
}

impl Auth {
    pub fn nueva(usuarios: Arc<Usuarios>, sesiones: Arc<Sesiones>, config: &Config) -> Self {
        Self {
            usuarios,
            sesiones,
            secreto: config.jwt_secret.clone(),
            access_ttl: config.access_ttl_segundos(),
            refresh_ttl: config.refresh_ttl_segundos(),
            exigir_correo_verificado: config.exigir_correo_verificado,
        }
    }

    pub async fn registrar(&self, datos: CrearUsuario) -> Result<Sesion, ApiError> {
        let usuario = self.registrar_usuario(datos).await?;
        self.sesion_de(&usuario, None, None).await
    }

    /// Registra sin emitir sesión. Se usa cuando la verificación de correo es
    /// obligatoria: el usuario debe verificar antes de recibir tokens.
    pub async fn registrar_usuario(&self, datos: CrearUsuario) -> Result<Usuario, ApiError> {
        if !ROLES_REGISTRABLES.contains(&datos.rol.as_str()) {
            return Err(ApiError::dominio(
                "ROL_NO_PERMITIDO",
                "El registro publico solo admite empresario o comprador.",
            ));
        }
        self.usuarios.crear(datos).await
    }

    pub async fn login(&self, correo: &str, contrasena: &str) -> Result<Sesion, ApiError> {
        self.login_con_mfa(correo, contrasena, None, None, None)
            .await
    }

    pub async fn login_con_mfa(
        &self,
        correo: &str,
        contrasena: &str,
        codigo_mfa: Option<&str>,
        user_agent: Option<&str>,
        ip: Option<&str>,
    ) -> Result<Sesion, ApiError> {
        let usuario = self
            .usuarios
            .verificar_credenciales(correo, contrasena)
            .await?;
        self.validar_empresa_activa(&usuario).await?;
        self.validar_correo_verificado(&usuario).await?;
        let seguridad = self.usuarios.estado_seguridad(&usuario.id).await?;
        if seguridad
            .get("mfa_habilitado")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            let Some(codigo) = codigo_mfa.filter(|codigo| !codigo.trim().is_empty()) else {
                return Err(ApiError::nueva(
                    "MFA_REQUERIDO",
                    "Esta cuenta requiere un codigo MFA.",
                    StatusCode::UNAUTHORIZED,
                )
                .con_detalles(serde_json::json!({ "mfa_requerido": true })));
            };
            if !self.usuarios.verificar_mfa(&usuario.id, codigo).await? {
                return Err(ApiError::nueva(
                    "MFA_INVALIDO",
                    "El codigo MFA no es valido.",
                    StatusCode::UNAUTHORIZED,
                ));
            }
        }
        self.sesion_de(&usuario, user_agent, ip).await
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
        let mut usuario = usuario;
        self.validar_empresa_activa(&usuario).await?;
        self.validar_correo_verificado(&usuario).await?;
        self.sesiones.actualizar_uso(jti).await?;
        if let Some(negocio) = sesion.negocio_id.clone() {
            usuario.negocio_id = Some(negocio);
        }
        self.emitir(&usuario, jti)
    }

    /// Cambia el negocio activo de una sesión y reemite los tokens.
    pub async fn cambiar_negocio(
        &self,
        usuario_id: &str,
        refresh_token: &str,
        negocio_id: &str,
    ) -> Result<Sesion, ApiError> {
        if Uuid::parse_str(negocio_id).is_err() {
            return Err(ApiError::dominio(
                "NEGOCIO_INVALIDO",
                "El id de negocio no es un UUID valido.",
            ));
        }
        let payload = verificar_token(refresh_token, &self.secreto)
            .map_err(|_| ApiError::no_autorizado("El refresh_token es invalido o expiro."))?;
        if payload.tipo != "refresh" || payload.sub != usuario_id {
            return Err(ApiError::no_autorizado(
                "El refresh_token no pertenece a esta sesion.",
            ));
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
        if sesion.usuario_id != usuario_id {
            return Err(ApiError::no_autorizado(
                "La sesion fue cerrada; inicia sesion de nuevo.",
            ));
        }
        let Some(mut usuario) = self.usuarios.encontrar_por_id(usuario_id).await? else {
            return Err(ApiError::no_autorizado(
                "El usuario del token ya no existe.",
            ));
        };
        self.validar_empresa_activa(&usuario).await?;
        self.validar_correo_verificado(&usuario).await?;
        let Some(empresa_id) = usuario.tenant_id.as_deref() else {
            return Err(ApiError::dominio(
                "TENANT_REQUERIDO",
                "El usuario no tiene una empresa activa.",
            ));
        };
        if !self
            .usuarios
            .negocio_accesible(usuario_id, empresa_id, negocio_id)
            .await?
        {
            return Err(ApiError::acceso_denegado(
                "El negocio no existe o no está asignado a tu usuario.",
            ));
        }
        if !self
            .sesiones
            .actualizar_negocio(jti, usuario_id, negocio_id)
            .await?
        {
            return Err(ApiError::no_autorizado(
                "La sesion fue cerrada; inicia sesion de nuevo.",
            ));
        }
        usuario.negocio_id = Some(negocio_id.to_string());
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

    pub async fn marcar_correo_verificado(&self, usuario_id: &str) -> Result<(), ApiError> {
        self.usuarios.marcar_correo_verificado(usuario_id).await
    }

    pub async fn listar_sesiones(&self, usuario_id: &str) -> Result<Vec<Value>, ApiError> {
        self.sesiones.listar_activas(usuario_id).await
    }

    pub async fn revocar_sesion(&self, usuario_id: &str, id: &str) -> Result<bool, ApiError> {
        self.sesiones.revocar_por_id(usuario_id, id).await
    }

    /// Crea una sesión para un usuario ya autenticado (aceptar invitación).
    pub async fn crear_sesion(&self, usuario: &Usuario) -> Result<Sesion, ApiError> {
        self.validar_empresa_activa(usuario).await?;
        self.validar_correo_verificado(usuario).await?;
        self.sesion_de(usuario, None, None).await
    }

    async fn validar_correo_verificado(&self, usuario: &Usuario) -> Result<(), ApiError> {
        if self.exigir_correo_verificado && !self.usuarios.correo_verificado(&usuario.id).await? {
            return Err(ApiError::nueva(
                "CORREO_NO_VERIFICADO",
                "Verifica tu correo antes de iniciar sesion.",
                StatusCode::FORBIDDEN,
            ));
        }
        Ok(())
    }

    /// Un tenant suspendido o cerrado no puede iniciar ni refrescar sesiones.
    async fn validar_empresa_activa(&self, usuario: &Usuario) -> Result<(), ApiError> {
        let Some(tenant_id) = usuario.tenant_id.as_deref() else {
            return Ok(());
        };
        match self.usuarios.estado_empresa(tenant_id).await?.as_deref() {
            Some("activa") => Ok(()),
            Some("suspendida") => Err(ApiError::nueva(
                "EMPRESA_SUSPENDIDA",
                "La empresa esta suspendida. Contacta al administrador.",
                StatusCode::FORBIDDEN,
            )),
            Some("inactiva") => Err(ApiError::nueva(
                "EMPRESA_INACTIVA",
                "La empresa esta cerrada.",
                StatusCode::FORBIDDEN,
            )),
            _ => Err(ApiError::nueva(
                "EMPRESA_NO_DISPONIBLE",
                "La empresa del usuario no esta disponible.",
                StatusCode::FORBIDDEN,
            )),
        }
    }

    async fn sesion_de(
        &self,
        usuario: &Usuario,
        user_agent: Option<&str>,
        ip: Option<&str>,
    ) -> Result<Sesion, ApiError> {
        let jti = Uuid::new_v4().to_string();
        let expira_en = Utc::now() + Duration::seconds(self.refresh_ttl as i64);
        self.sesiones
            .crear(
                &usuario.id,
                &jti,
                expira_en,
                usuario.negocio_id.as_deref(),
                user_agent,
                ip,
            )
            .await?;
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
        negocio_id: usuario.negocio_id.clone(),
        permisos: usuario.permisos.clone(),
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
            negocio_id: Some("n-1".into()),
            permisos: vec!["ordenes.leer".into()],
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
