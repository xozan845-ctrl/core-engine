//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::config::Config;
use crate::db::Db;
use crate::metrics::Metrics;
use crate::plantillas_db::Plantillas;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub db: Option<Arc<Db>>,
    pub plantillas: Option<Arc<Plantillas>>,
}

impl AppState {
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            db: None,
            plantillas: None,
        }
    }

    pub fn plantillas(&self) -> Result<&Arc<Plantillas>, axum::http::StatusCode> {
        self.plantillas
            .as_ref()
            .ok_or(axum::http::StatusCode::SERVICE_UNAVAILABLE)
    }
}
