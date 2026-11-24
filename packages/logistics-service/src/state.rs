//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible, UsuarioContexto};
use crate::config::Config;
use crate::db::Db;
use crate::envios::{EnviosNoDisponible, EnviosPort};
use crate::errors::ApiError;
use crate::metrics::Metrics;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub envios: Arc<dyn EnviosPort>,
    pub capacidades: Arc<dyn Capacidades>,
    pub db: Option<Arc<Db>>,
    pub inicio: Instant,
}

impl AppState {
    /// Estado sin dependencias externas (rutas de infraestructura y tests).
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            envios: Arc::new(EnviosNoDisponible),
            capacidades: Arc::new(CapacidadesNoDisponible),
            db: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        envios: Arc<dyn EnviosPort>,
        capacidades: Arc<dyn Capacidades>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            envios,
            capacidades,
            db: None,
            inicio: Instant::now(),
        }
    }

    pub fn con_db(mut self, db: Arc<Db>) -> Self {
        self.db = Some(db);
        self
    }

    pub fn db(&self) -> Result<&Arc<Db>, ApiError> {
        self.db.as_ref().ok_or_else(ApiError::interno)
    }

    /// Equivalente a `CapabilityGuard`: admin global pasa; el resto exige
    /// tenant y consulta identity con fallo cerrado.
    pub async fn exigir_capacidades(
        &self,
        usuario: &UsuarioContexto,
        requeridas: &[&str],
    ) -> Result<(), ApiError> {
        if requeridas.is_empty() || usuario.es_admin_global() {
            return Ok(());
        }
        let tenant = usuario.tenant()?;
        let capacidades = self
            .capacidades
            .activas_en(tenant, usuario.negocio_id.as_deref(), requeridas)
            .await
            .map_err(|_| ApiError::servicio_no_disponible("identity-service"))?;
        let activas: Vec<&str> = capacidades
            .iter()
            .filter(|fila| fila.activo && fila.empresa_id == tenant)
            .map(|fila| fila.capacidad_codigo.as_str())
            .collect();
        let faltantes: Vec<&str> = requeridas
            .iter()
            .copied()
            .filter(|capacidad| !activas.contains(capacidad))
            .collect();
        if !faltantes.is_empty() {
            return Err(ApiError::capacidad_no_activa(&faltantes, tenant));
        }
        Ok(())
    }
}
