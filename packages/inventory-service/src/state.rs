//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::config::Config;
use crate::db::Db;
use crate::inventario::Inventario;
use crate::metrics::Metrics;
use crate::movimientos::Movimientos;
use crate::ubicaciones::Ubicaciones;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub capacidades: Arc<dyn Capacidades>,
    /// `None` solo en el estado de pruebas de infraestructura; las rutas de
    /// negocio fallarán con 500 si no hay base de datos.
    pub db: Option<Arc<Db>>,
    pub inventario: Option<Arc<Inventario>>,
    pub ubicaciones: Option<Arc<Ubicaciones>>,
    pub movimientos: Option<Arc<Movimientos>>,
    pub inicio: Instant,
}

impl AppState {
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            capacidades: Arc::new(CapacidadesNoDisponible),
            db: None,
            inventario: None,
            ubicaciones: None,
            movimientos: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_db(
        config: Config,
        metrics: Arc<Metrics>,
        db: Arc<Db>,
        capacidades: Arc<dyn Capacidades>,
    ) -> Self {
        let inventario = Arc::new(Inventario::nueva(Arc::clone(&db)));
        let ubicaciones = Arc::new(Ubicaciones::nueva(Arc::clone(&db)));
        let movimientos = Arc::new(Movimientos::nueva(Arc::clone(&db)));
        Self {
            config: Arc::new(config),
            metrics,
            capacidades,
            db: Some(db),
            inventario: Some(inventario),
            ubicaciones: Some(ubicaciones),
            movimientos: Some(movimientos),
            inicio: Instant::now(),
        }
    }

    pub fn db(&self) -> Result<&Arc<Db>, crate::errors::ApiError> {
        self.db
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn inventario(&self) -> Result<&Arc<Inventario>, crate::errors::ApiError> {
        self.inventario
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn ubicaciones(&self) -> Result<&Arc<Ubicaciones>, crate::errors::ApiError> {
        self.ubicaciones
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn movimientos(&self) -> Result<&Arc<Movimientos>, crate::errors::ApiError> {
        self.movimientos
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
            Config::from_pairs([("SERVICIO", "inventory-service")]).expect("config"),
            Metrics::nueva("inventory-service"),
        );
        let error = match state.db() {
            Ok(_) => panic!("sin base de datos"),
            Err(error) => error,
        };
        assert_eq!(error.codigo, "ERROR_INTERNO");
    }
}
