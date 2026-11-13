//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible, UsuarioContexto};
use crate::config::Config;
use crate::errors::ApiError;
use crate::inteligencia::{InteligenciaNoDisponible, InteligenciaPort};
use crate::metrics::Metrics;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub inteligencia: Arc<dyn InteligenciaPort>,
    pub capacidades: Arc<dyn Capacidades>,
    pub inicio: Instant,
}

impl AppState {
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            inteligencia: Arc::new(InteligenciaNoDisponible),
            capacidades: Arc::new(CapacidadesNoDisponible),
            inicio: Instant::now(),
        }
    }

    pub fn con_dependencias(
        config: Config,
        metrics: Arc<Metrics>,
        inteligencia: Arc<dyn InteligenciaPort>,
        capacidades: Arc<dyn Capacidades>,
    ) -> Self {
        Self {
            config: Arc::new(config),
            metrics,
            inteligencia,
            capacidades,
            inicio: Instant::now(),
        }
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
            .activas(tenant, requeridas)
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
