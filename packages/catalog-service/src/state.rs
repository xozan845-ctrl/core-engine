//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::config::Config;
use crate::db::Db;
use crate::inventario::Inventario;
use crate::items::Items;
use crate::metrics::Metrics;
use crate::ubicaciones::Ubicaciones;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub capacidades: Arc<dyn Capacidades>,
    /// `None` solo en el estado de pruebas de infraestructura; las rutas de
    /// negocio fallan con 500 si no hay base de datos.
    pub db: Option<Arc<Db>>,
    pub items: Option<Arc<Items>>,
    pub inventario: Option<Arc<Inventario>>,
    pub ubicaciones: Option<Arc<Ubicaciones>>,
    pub inicio: Instant,
}

impl AppState {
    /// Estado sin dependencias externas (rutas de infraestructura y tests).
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            capacidades: Arc::new(CapacidadesNoDisponible),
            db: None,
            items: None,
            inventario: None,
            ubicaciones: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        db: Arc<Db>,
        capacidades: Arc<dyn Capacidades>,
        items: Arc<Items>,
        inventario: Arc<Inventario>,
        ubicaciones: Arc<Ubicaciones>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            capacidades,
            items: Some(items),
            inventario: Some(inventario),
            ubicaciones: Some(ubicaciones),
            db: Some(db),
            inicio: Instant::now(),
        }
    }

    pub fn items(&self) -> Result<&Arc<Items>, crate::errors::ApiError> {
        self.items
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

    pub fn db(&self) -> Result<&Arc<Db>, crate::errors::ApiError> {
        self.db
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }
}
