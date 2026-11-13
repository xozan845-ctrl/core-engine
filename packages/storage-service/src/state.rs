//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::config::Config;
use crate::metrics::Metrics;
use crate::storage::buckets::Buckets;
use crate::storage::objetos::Objetos;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub buckets: Option<Arc<Buckets>>,
    pub objetos: Option<Arc<Objetos>>,
    pub inicio: Instant,
}

impl AppState {
    /// Estado sin dependencias externas (rutas de infraestructura y tests).
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            buckets: None,
            objetos: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        buckets: Arc<Buckets>,
        objetos: Arc<Objetos>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            buckets: Some(buckets),
            objetos: Some(objetos),
            inicio: Instant::now(),
        }
    }

    pub fn buckets(&self) -> Result<&Arc<Buckets>, crate::errors::ApiError> {
        self.buckets
            .as_ref()
            .ok_or_else(|| crate::errors::ApiError::servicio_no_disponible("storage-service"))
    }

    pub fn objetos(&self) -> Result<&Arc<Objetos>, crate::errors::ApiError> {
        self.objetos
            .as_ref()
            .ok_or_else(|| crate::errors::ApiError::servicio_no_disponible("storage-service"))
    }
}
