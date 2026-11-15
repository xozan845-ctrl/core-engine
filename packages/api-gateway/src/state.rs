//! Estado compartido del gateway.

use crate::config::Config;
use crate::metrics::Metrics;
use crate::proxy::{destinos, Destino};
use crate::rate_limit::RateLimit;
use crate::readiness::EstadoReadiness;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub rate: Arc<RateLimit>,
    pub http: reqwest::Client,
    pub destinos: Arc<Vec<Destino>>,
    pub ready_cache: Arc<Mutex<Option<(Instant, EstadoReadiness)>>>,
}

impl AppState {
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        let destinos = Arc::new(destinos(&config));
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_default();
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            rate: Arc::new(RateLimit::nueva()),
            http,
            destinos,
            ready_cache: Arc::new(Mutex::new(None)),
        }
    }
}
