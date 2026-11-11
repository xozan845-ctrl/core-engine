//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::config::Config;
use crate::metrics::Metrics;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub inicio: Instant,
}

impl AppState {
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            inicio: Instant::now(),
        }
    }
}
