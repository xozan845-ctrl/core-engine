//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::config::Config;
use crate::dominio::auth::Auth;
use crate::dominio::capacidades::Capacidades;
use crate::dominio::packs::Packs;
use crate::dominio::password_reset::PasswordReset;
use crate::dominio::usuarios::Usuarios;
use crate::metrics::Metrics;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct Servicios {
    pub auth: Arc<Auth>,
    pub usuarios: Arc<Usuarios>,
    pub capacidades: Arc<Capacidades>,
    pub packs: Arc<Packs>,
    pub password_reset: Arc<PasswordReset>,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub inicio: Instant,
    pub servicios: Option<Arc<Servicios>>,
}

impl AppState {
    /// Estado sin base de datos (rutas de infraestructura y tests de router).
    pub fn nueva(config: Config, metrics: Arc<Metrics>) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            inicio: Instant::now(),
            servicios: None,
        }
    }

    pub fn con_servicios(config: Config, metrics: Arc<Metrics>, servicios: Servicios) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            inicio: Instant::now(),
            servicios: Some(Arc::new(servicios)),
        }
    }

    pub fn servicios(&self) -> Result<&Arc<Servicios>, crate::errors::ApiError> {
        self.servicios
            .as_ref()
            .ok_or_else(|| crate::errors::ApiError::servicio_no_disponible("identity-service"))
    }
}
