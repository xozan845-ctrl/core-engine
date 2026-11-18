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
    pub ordenes: Arc<dyn crate::clients::Ordenes>,
    pub db: Option<Arc<crate::db::Db>>,
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
            ordenes: Arc::new(crate::clients::OrdenesSinConexion),
            db: None,
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
        ordenes: Arc<dyn crate::clients::Ordenes>,
        db: Arc<crate::db::Db>,
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
            ordenes,
            db: Some(db),
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

    pub fn db(&self) -> Result<&Arc<crate::db::Db>, crate::errors::ApiError> {
        self.db
            .as_ref()
            .ok_or_else(crate::errors::ApiError::interno)
    }

    pub fn resenas(&self) -> Result<crate::resenas::Resenas, crate::errors::ApiError> {
        Ok(crate::resenas::Resenas::nueva(
            Arc::clone(self.db()?),
            Arc::clone(&self.catalogo),
            Arc::clone(&self.capacidades),
            Arc::clone(&self.ordenes),
        ))
    }

    pub fn favoritos(&self) -> Result<crate::favoritos::Favoritos, crate::errors::ApiError> {
        Ok(crate::favoritos::Favoritos::nueva(
            Arc::clone(self.db()?),
            Arc::clone(&self.catalogo),
        ))
    }
}
