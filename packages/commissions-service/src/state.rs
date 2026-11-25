//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::comisiones::{ComisionesNoDisponible, ComisionesPort};
use crate::config::Config;
use crate::db::Db;
use crate::dispersion::Dispersion;
use crate::errors::ApiError;
use crate::liquidaciones::{LiquidacionesNoDisponible, LiquidacionesPort};
use crate::metrics::Metrics;
use crate::reportes::{ReportesNoDisponible, ReportesPort};
use crate::ventas::{VentasNoDisponible, VentasPort};
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub comisiones: Arc<dyn ComisionesPort>,
    pub liquidaciones: Arc<dyn LiquidacionesPort>,
    pub ventas: Arc<dyn VentasPort>,
    pub reportes: Arc<dyn ReportesPort>,
    pub capacidades: Arc<dyn Capacidades>,
    pub dispersion: Option<Arc<Dispersion>>,
    pub db: Option<Arc<Db>>,
    pub inicio: Instant,
}

impl AppState {
    /// Estado sin dependencias externas (rutas de infraestructura y tests).
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            comisiones: Arc::new(ComisionesNoDisponible),
            liquidaciones: Arc::new(LiquidacionesNoDisponible),
            ventas: Arc::new(VentasNoDisponible),
            reportes: Arc::new(ReportesNoDisponible),
            capacidades: Arc::new(CapacidadesNoDisponible),
            dispersion: None,
            db: None,
            inicio: Instant::now(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        comisiones: Arc<dyn ComisionesPort>,
        liquidaciones: Arc<dyn LiquidacionesPort>,
        ventas: Arc<dyn VentasPort>,
        reportes: Arc<dyn ReportesPort>,
        capacidades: Arc<dyn Capacidades>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            comisiones,
            liquidaciones,
            ventas,
            reportes,
            capacidades,
            dispersion: None,
            db: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_db(mut self, db: Arc<Db>) -> Self {
        self.db = Some(db);
        self
    }

    pub fn con_dispersion(mut self, dispersion: Arc<Dispersion>) -> Self {
        self.dispersion = Some(dispersion);
        self
    }

    pub fn dispersion(&self) -> Result<&Arc<Dispersion>, ApiError> {
        self.dispersion
            .as_ref()
            .ok_or_else(|| ApiError::servicio_no_disponible("dispersion"))
    }

    pub fn db(&self) -> Result<&Arc<Db>, ApiError> {
        self.db.as_ref().ok_or_else(ApiError::interno)
    }
}
