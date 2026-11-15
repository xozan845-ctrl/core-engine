//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::config::Config;
use crate::field::FieldService;
use crate::metrics::Metrics;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    /// `None` solo en el estado de pruebas de infraestructura; las rutas de
    /// negocio fallan con 500 si no hay base de datos.
    pub field: Option<Arc<FieldService>>,
    pub capacidades: Arc<dyn Capacidades>,
    pub inicio: Instant,
}

impl AppState {
    /// Estado sin dependencias externas (rutas de infraestructura y tests).
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            field: None,
            capacidades: Arc::new(CapacidadesNoDisponible),
            inicio: Instant::now(),
        }
    }

    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        field: Arc<FieldService>,
        capacidades: Arc<dyn Capacidades>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            field: Some(field),
            capacidades,
            inicio: Instant::now(),
        }
    }

    pub fn field(&self) -> Result<&Arc<FieldService>, crate::errors::ApiError> {
        self.field
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }
}
