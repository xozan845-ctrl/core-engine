//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::carrito::Carrito;
use crate::checkout::Checkout;
use crate::clients::ClientesPort;
use crate::config::Config;
use crate::contratos::Contratos;
use crate::db::Db;
use crate::entregas::EntregasDigitales;
use crate::errors::ApiError;
use crate::metrics::Metrics;
use crate::pedidos::Pedidos;
use crate::salon::Salon;
use crate::suscripciones::Suscripciones;
use crate::views::OrderViewRepository;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    dbs: Option<Arc<Db>>,
    clientes: Option<Arc<dyn ClientesPort>>,
    pub checkout: Option<Arc<Checkout>>,
    pub carrito: Option<Arc<Carrito>>,
    pub vistas: Option<Arc<OrderViewRepository>>,
    pub entregas: Option<Arc<EntregasDigitales>>,
    pub pedidos: Option<Arc<Pedidos>>,
    pub contratos: Option<Arc<Contratos>>,
    pub suscripciones: Option<Arc<Suscripciones>>,
    pub salon: Option<Arc<Salon>>,
    pub inicio: Instant,
}

impl AppState {
    /// Estado sin dependencias externas (rutas de infraestructura y tests).
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            dbs: None,
            clientes: None,
            checkout: None,
            carrito: None,
            vistas: None,
            entregas: None,
            pedidos: None,
            contratos: None,
            suscripciones: None,
            salon: None,
            inicio: Instant::now(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        db: Arc<Db>,
        clientes: Arc<dyn ClientesPort>,
        checkout: Arc<Checkout>,
        carrito: Arc<Carrito>,
        vistas: Arc<OrderViewRepository>,
        entregas: Arc<EntregasDigitales>,
        pedidos: Arc<Pedidos>,
        contratos: Arc<Contratos>,
        suscripciones: Arc<Suscripciones>,
        salon: Arc<Salon>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            dbs: Some(db),
            clientes: Some(clientes),
            checkout: Some(checkout),
            carrito: Some(carrito),
            vistas: Some(vistas),
            entregas: Some(entregas),
            pedidos: Some(pedidos),
            contratos: Some(contratos),
            suscripciones: Some(suscripciones),
            salon: Some(salon),
            inicio: Instant::now(),
        }
    }

    pub fn db(&self) -> Result<&Arc<Db>, ApiError> {
        self.dbs.as_ref().ok_or_else(ApiError::interno)
    }

    pub fn clientes(&self) -> Result<&Arc<dyn ClientesPort>, ApiError> {
        self.clientes.as_ref().ok_or_else(ApiError::interno)
    }

    pub fn checkout(&self) -> Result<&Arc<Checkout>, ApiError> {
        self.checkout.as_ref().ok_or_else(ApiError::interno)
    }

    pub fn carrito(&self) -> Result<&Arc<Carrito>, ApiError> {
        self.carrito.as_ref().ok_or_else(ApiError::interno)
    }

    pub fn vistas(&self) -> Result<&Arc<OrderViewRepository>, ApiError> {
        self.vistas.as_ref().ok_or_else(ApiError::interno)
    }

    pub fn entregas(&self) -> Result<&Arc<EntregasDigitales>, ApiError> {
        self.entregas.as_ref().ok_or_else(ApiError::interno)
    }

    pub fn pedidos(&self) -> Result<&Arc<Pedidos>, ApiError> {
        self.pedidos.as_ref().ok_or_else(ApiError::interno)
    }

    pub fn contratos(&self) -> Result<&Arc<Contratos>, ApiError> {
        self.contratos.as_ref().ok_or_else(ApiError::interno)
    }

    pub fn suscripciones(&self) -> Result<&Arc<Suscripciones>, ApiError> {
        self.suscripciones.as_ref().ok_or_else(ApiError::interno)
    }

    pub fn salon(&self) -> Result<&Arc<Salon>, ApiError> {
        self.salon.as_ref().ok_or_else(ApiError::interno)
    }
}
