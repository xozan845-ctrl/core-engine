//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::config::Config;
use crate::dominio::api_keys::ApiKeys;
use crate::dominio::auth::Auth;
use crate::dominio::capacidades::Capacidades;
use crate::dominio::configuracion_sitio::ConfiguracionSitio;
use crate::dominio::empresas::Empresas;
use crate::dominio::extensiones::Extensiones;
use crate::dominio::packs::Packs;
use crate::dominio::partes::Partes;
use crate::dominio::password_reset::PasswordReset;
use crate::dominio::personal::Personal;
use crate::dominio::registros::Registros;
use crate::dominio::usuarios::Usuarios;
use crate::metrics::Metrics;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct Servicios {
    pub auth: Arc<Auth>,
    pub usuarios: Arc<Usuarios>,
    pub api_keys: Arc<ApiKeys>,
    pub capacidades: Arc<Capacidades>,
    pub configuracion: Arc<ConfiguracionSitio>,
    pub empresas: Arc<Empresas>,
    pub extensiones: Arc<Extensiones>,
    pub personal: Arc<Personal>,
    pub partes: Arc<Partes>,
    pub registros: Arc<Registros>,
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
