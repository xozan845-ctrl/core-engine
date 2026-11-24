//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::config::Config;
use crate::db::Db;
use crate::metrics::Metrics;
use crate::plantillas_db::Plantillas;
use crate::webhooks::Webhooks;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub db: Option<Arc<Db>>,
    pub plantillas: Option<Arc<Plantillas>>,
    pub webhooks: Option<Arc<Webhooks>>,
}

impl AppState {
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            db: None,
            plantillas: None,
            webhooks: None,
        }
    }

    pub fn plantillas(&self) -> Result<&Arc<Plantillas>, axum::http::StatusCode> {
        self.plantillas
            .as_ref()
            .ok_or(axum::http::StatusCode::SERVICE_UNAVAILABLE)
    }

    pub fn webhooks(&self) -> Result<&Arc<Webhooks>, axum::http::StatusCode> {
        self.webhooks
            .as_ref()
            .ok_or(axum::http::StatusCode::SERVICE_UNAVAILABLE)
    }
}
