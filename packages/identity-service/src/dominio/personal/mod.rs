//! Personal: roles, puestos, asignaciones, invitaciones e incorporaciones.
//!
//! Réplica de `personal/personal.service.ts`. Las tiendas son autoridad de
//! `stores-service`; identity solo guarda la referencia y las valida por API
//! interna.

mod asignaciones;
mod comunes;
mod contexto;
mod invitaciones;
mod roles;

mod prelude {
    pub(crate) use crate::auth::{hash_contrasena, UsuarioContexto};
    pub(crate) use crate::db::{iso_ms, Db};
    pub(crate) use crate::dominio::auditoria;
    pub(crate) use crate::dominio::auth::Auth;
    pub(crate) use crate::dominio::negocios::Negocios;
    pub(crate) use crate::dominio::usuarios::{correo_valido, Usuario};
    pub(crate) use crate::email::{Email, EmailMessage};
    pub(crate) use crate::errors::ApiError;
    pub(crate) use crate::stores::Stores;
    pub(crate) use axum::http::StatusCode;
    pub(crate) use base64::Engine;
    pub(crate) use chrono::{DateTime, Duration, Utc};
    pub(crate) use rand::Rng;
    pub(crate) use serde_json::{json, Map, Value};
    pub(crate) use sha2::{Digest, Sha256};
    pub(crate) use sqlx::{Postgres, Row, Transaction};
    pub(crate) use std::collections::HashMap;
    pub(crate) use std::sync::Arc;
    pub(crate) use uuid::Uuid;
}
use self::prelude::*;

const TOKEN_TTL_HORAS: i64 = 72;
const CODIGO: &str = "código";

pub struct CrearRol {
    pub codigo: String,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub permisos: Value,
    pub negocio_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CrearPuesto {
    pub codigo: String,
    pub nombre: String,
    pub tipo: Option<String>,
    pub tienda_id: Option<String>,
    pub negocio_id: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct AsignarPersonal {
    pub usuario_id: String,
    pub rol_empresa_id: String,
    pub tienda_id: Option<String>,
    pub puesto_id: Option<String>,
    pub negocio_id: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct CrearInvitacion {
    pub correo: String,
    pub nombre: Option<String>,
    pub tipo: Option<String>,
    pub rol_empresa_id: String,
    pub tienda_id: Option<String>,
    pub puesto_id: Option<String>,
    pub negocio_id: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct TransferirPersonal {
    pub usuario_id: String,
    pub origen_id: String,
    pub destino_id: String,
    pub rol_empresa_id: String,
    pub tienda_id: Option<String>,
    pub puesto_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AceptarInvitacion {
    pub nombre: String,
    pub contrasena: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccionPersonal {
    Leer,
    Gestionar,
}

impl AccionPersonal {
    fn permiso(self) -> &'static str {
        match self {
            Self::Leer => "personal.leer",
            Self::Gestionar => "personal.gestionar",
        }
    }
}

#[derive(Debug, Clone)]
struct AsignacionActor {
    negocio_id: Option<String>,
    rol_negocio_id: Option<String>,
    codigo: String,
    permisos: Vec<String>,
}

/// Alcance efectivo del actor dentro de una empresa. `empresa_global` solo es
/// cierto para un `empresario` con rol transversal o un admin de plataforma.
#[derive(Debug, Clone)]
struct ContextoPersonal {
    empresa_global: bool,
    negocio_id: Option<String>,
    asignaciones: Vec<AsignacionActor>,
}

impl ContextoPersonal {
    fn autorizado(&self, accion: AccionPersonal, negocio_id: Option<&str>) -> bool {
        self.asignaciones.iter().any(|asignacion| {
            let alcance = match (asignacion.rol_negocio_id.as_deref(), negocio_id) {
                (None, _) => true,
                (Some(rol), Some(objetivo)) => rol == objetivo,
                (Some(_), None) => false,
            };
            alcance
                && (asignacion.codigo == "empresario"
                    || asignacion
                        .permisos
                        .iter()
                        .any(|permiso| permiso == accion.permiso()))
        })
    }

    fn puede_leer(&self) -> bool {
        self.asignaciones.iter().any(|asignacion| {
            asignacion.codigo == "empresario"
                || asignacion
                    .permisos
                    .iter()
                    .any(|permiso| permiso == AccionPersonal::Leer.permiso())
        })
    }

    /// Permisos que el actor puede otorgar en el alcance indicado.
    fn permisos_en(&self, negocio_id: Option<&str>) -> Vec<String> {
        self.asignaciones
            .iter()
            .filter(
                |asignacion| match (asignacion.rol_negocio_id.as_deref(), negocio_id) {
                    (None, _) => true,
                    (Some(rol), Some(objetivo)) => rol == objetivo,
                    (Some(_), None) => false,
                },
            )
            .flat_map(|asignacion| asignacion.permisos.clone())
            .collect()
    }
}

pub struct Personal {
    db: Arc<Db>,
    negocios: Arc<Negocios>,
    auth: Arc<Auth>,
    email: Arc<dyn Email>,
    stores: Arc<Stores>,
    frontend_url: String,
}

impl Personal {
    pub fn nueva(
        db: Arc<Db>,
        negocios: Arc<Negocios>,
        auth: Arc<Auth>,
        email: Arc<dyn Email>,
        stores: Arc<Stores>,
        frontend_url: String,
    ) -> Self {
        Self {
            db,
            negocios,
            auth,
            email,
            stores,
            frontend_url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::comunes::{
        hash_token, token_aleatorio, validar_codigo, validar_correo_invitacion, validar_permisos,
        validar_vigente,
    };
    use serde_json::json;

    #[test]
    fn debe_validar_permisos_y_deduplicar() {
        let permisos = validar_permisos(&json!({
            "acciones": ["catalogo.leer", "catalogo.leer", "ordenes.crear"],
            "recursos": ["tienda"]
        }))
        .expect("permisos");
        assert_eq!(permisos["acciones"].as_array().expect("acciones").len(), 2);
        assert_eq!(permisos["version"], 1);
        assert_eq!(permisos["recursos"], json!(["tienda"]));
        assert!(validar_permisos(&json!({"acciones": []})).is_err());
        assert!(validar_permisos(&json!({"acciones": ["MAL"]})).is_err());
    }

    #[test]
    fn debe_generar_token_y_hash_estables() {
        let token = token_aleatorio();
        assert_eq!(token.len(), 43);
        assert_eq!(hash_token(&token).len(), 64);
        assert_ne!(token, token_aleatorio());
    }

    #[test]
    fn debe_validar_correos_y_codigos() {
        assert!(validar_correo_invitacion("ana@tienda.test").is_ok());
        assert!(validar_correo_invitacion("ana@localhost").is_err());
        assert_eq!(
            validar_codigo("Estilista", "codigo").expect("codigo"),
            "estilista"
        );
        assert!(validar_codigo("1malo", "codigo").is_err());
    }

    #[test]
    fn debe_rechazar_una_invitacion_expirada() {
        let datos = json!({"estado": "pendiente", "expira_en": "2020-01-01T00:00:00Z"});
        assert_eq!(
            validar_vigente(&datos).expect_err("expirada").codigo,
            "INVITACION_NO_VALIDA"
        );
    }
}
