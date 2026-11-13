//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::clients::{
    Catalogo, CatalogoSinConexion, ObjetosStorage, StorageSinConexion, TemasIdentity,
    TemasSinHerencia,
};
use crate::config::Config;
use crate::metrics::Metrics;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub capacidades: Arc<dyn Capacidades>,
    pub temas: Arc<dyn TemasIdentity>,
    pub storage: Arc<dyn ObjetosStorage>,
    pub catalogo: Arc<dyn Catalogo>,
    pub inicio: Instant,
}

impl AppState {
    /// Estado sin dependencias externas (rutas de infraestructura y tests).
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            capacidades: Arc::new(CapacidadesNoDisponible),
            temas: Arc::new(TemasSinHerencia),
            storage: Arc::new(StorageSinConexion),
            catalogo: Arc::new(CatalogoSinConexion),
            inicio: Instant::now(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        capacidades: Arc<dyn Capacidades>,
        temas: Arc<dyn TemasIdentity>,
        storage: Arc<dyn ObjetosStorage>,
        catalogo: Arc<dyn Catalogo>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            capacidades,
            temas,
            storage,
            catalogo,
            inicio: Instant::now(),
        }
    }
}
