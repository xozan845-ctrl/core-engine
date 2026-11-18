//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::config::Config;
use crate::cupones::Cupones;
use crate::db::Db;
use crate::metrics::Metrics;
use crate::precios::Precios;
use crate::promociones::Promociones;
use crate::tarifas::Tarifas;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub capacidades: Arc<dyn Capacidades>,
    pub db: Option<Arc<Db>>,
    pub precios: Option<Arc<Precios>>,
    pub tarifas: Option<Arc<Tarifas>>,
    pub promociones: Option<Arc<Promociones>>,
    pub cupones: Option<Arc<Cupones>>,
    pub inicio: Instant,
}

impl AppState {
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            capacidades: Arc::new(CapacidadesNoDisponible),
            db: None,
            precios: None,
            tarifas: None,
            promociones: None,
            cupones: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_db(
        config: Config,
        metrics: Arc<Metrics>,
        db: Arc<Db>,
        capacidades: Arc<dyn Capacidades>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            capacidades,
            precios: Some(Arc::new(Precios::nueva(Arc::clone(&db)))),
            tarifas: Some(Arc::new(Tarifas::nueva(Arc::clone(&db)))),
            promociones: Some(Arc::new(Promociones::nueva(Arc::clone(&db)))),
            cupones: Some(Arc::new(Cupones::nueva(Arc::clone(&db)))),
            db: Some(db),
            inicio: Instant::now(),
        }
    }

    pub fn db(&self) -> Result<&Arc<Db>, crate::errors::ApiError> {
        self.db
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn precios(&self) -> Result<&Arc<Precios>, crate::errors::ApiError> {
        self.precios
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn tarifas(&self) -> Result<&Arc<Tarifas>, crate::errors::ApiError> {
        self.tarifas
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn promociones(&self) -> Result<&Arc<Promociones>, crate::errors::ApiError> {
        self.promociones
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn cupones(&self) -> Result<&Arc<Cupones>, crate::errors::ApiError> {
        self.cupones
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_devolver_error_si_no_hay_base_de_datos() {
        let state = AppState::nueva(
            Config::from_pairs([("SERVICIO", "pricing-service")]).expect("config"),
            Metrics::nueva("pricing-service"),
        );
        let error = match state.db() {
            Ok(_) => panic!("sin base de datos"),
            Err(error) => error,
        };
        assert_eq!(error.codigo, "ERROR_INTERNO");
    }
}
