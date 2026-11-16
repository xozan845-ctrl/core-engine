//! Estado compartido del servicio.

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::config::Config;
use crate::db::Db;
use crate::errors::ApiError;
use crate::metrics::Metrics;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub capacidades: Arc<dyn Capacidades>,
    pub db: Option<Arc<Db>>,
    pub inicio: Instant,
}

impl AppState {
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            capacidades: Arc::new(CapacidadesNoDisponible),
            db: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        capacidades: Arc<dyn Capacidades>,
        db: Arc<Db>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            capacidades,
            db: Some(db),
            inicio: Instant::now(),
        }
    }

    pub fn db(&self) -> Result<&Arc<Db>, ApiError> {
        self.db
            .as_ref()
            .ok_or_else(|| ApiError::servicio_no_disponible("base de datos no configurada"))
    }
}
