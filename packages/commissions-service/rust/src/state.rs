//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::comisiones::{ComisionesNoDisponible, ComisionesPort};
use crate::config::Config;
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
            inicio: Instant::now(),
        }
    }
}
