//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::config::Config;
use crate::db::Db;
use crate::errors::ApiError;
use crate::metrics::Metrics;
use crate::scheduling::{AgendaNoDisponible, AgendaPort};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub agenda: Arc<dyn AgendaPort>,
    pub capacidades: Arc<dyn Capacidades>,
    pub db: Option<Arc<Db>>,
    pub inicio: Instant,
}

impl AppState {
    /// Estado sin dependencias externas (rutas de infraestructura y tests).
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            agenda: Arc::new(AgendaNoDisponible),
            capacidades: Arc::new(CapacidadesNoDisponible),
            db: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        agenda: Arc<dyn AgendaPort>,
        capacidades: Arc<dyn Capacidades>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            agenda,
            capacidades,
            db: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_db(mut self, db: Arc<Db>) -> Self {
        self.db = Some(db);
        self
    }

    pub fn db(&self) -> Result<&Arc<Db>, ApiError> {
        self.db.as_ref().ok_or_else(ApiError::interno)
    }
}
