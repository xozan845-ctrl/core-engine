//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::componentes::Componentes;
use crate::config::Config;
use crate::db::Db;
use crate::items::Items;
use crate::metrics::Metrics;
use crate::operaciones::Operaciones;
use crate::productos::Productos;
use crate::series::Series;
use crate::servicios::Servicios;
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
    pub productos: Option<Arc<Productos>>,
    pub servicios: Option<Arc<Servicios>>,
    pub series: Option<Arc<Series>>,
    pub componentes: Option<Arc<Componentes>>,
    pub operaciones: Option<Arc<Operaciones>>,
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
            productos: None,
            servicios: None,
            series: None,
            componentes: None,
            operaciones: None,
            inicio: Instant::now(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        db: Arc<Db>,
        capacidades: Arc<dyn Capacidades>,
        items: Arc<Items>,
        productos: Arc<Productos>,
        servicios: Arc<Servicios>,
        series: Arc<Series>,
        componentes: Arc<Componentes>,
        operaciones: Arc<Operaciones>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            capacidades,
            items: Some(items),
            productos: Some(productos),
            servicios: Some(servicios),
            series: Some(series),
            componentes: Some(componentes),
            operaciones: Some(operaciones),
            db: Some(db),
            inicio: Instant::now(),
        }
    }

    pub fn items(&self) -> Result<&Arc<Items>, crate::errors::ApiError> {
        self.items
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn productos(&self) -> Result<&Arc<Productos>, crate::errors::ApiError> {
        self.productos
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn servicios(&self) -> Result<&Arc<Servicios>, crate::errors::ApiError> {
        self.servicios
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn series(&self) -> Result<&Arc<Series>, crate::errors::ApiError> {
        self.series
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn componentes(&self) -> Result<&Arc<Componentes>, crate::errors::ApiError> {
        self.componentes
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn operaciones(&self) -> Result<&Arc<Operaciones>, crate::errors::ApiError> {
        self.operaciones
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn db(&self) -> Result<&Arc<Db>, crate::errors::ApiError> {
        self.db
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }
}
