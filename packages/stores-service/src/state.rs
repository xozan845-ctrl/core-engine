//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible};
use crate::clients::{
    Catalogo, CatalogoSinConexion, ObjetosStorage, StorageSinConexion, TemasIdentity,
    TemasSinHerencia,
};
use crate::config::Config;
use crate::metrics::Metrics;
use crate::ofertas::Ofertas;
use crate::tiendas::Tiendas;
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
    /// `None` solo en el estado de pruebas de infraestructura; las rutas de
    /// negocio fallan con 500 si no hay base de datos.
    pub tiendas: Option<Arc<Tiendas>>,
    pub ofertas: Option<Arc<Ofertas>>,
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
            tiendas: None,
            ofertas: None,
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
        tiendas: Arc<Tiendas>,
        ofertas: Arc<Ofertas>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            capacidades,
            temas,
            storage,
            catalogo,
            tiendas: Some(tiendas),
            ofertas: Some(ofertas),
            inicio: Instant::now(),
        }
    }

    pub fn tiendas(&self) -> Result<&Arc<Tiendas>, crate::errors::ApiError> {
        self.tiendas
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn ofertas(&self) -> Result<&Arc<Ofertas>, crate::errors::ApiError> {
        self.ofertas
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }
}
