//! Estado compartido del servicio (equivalente a la DI de NestJS, R-MS-3).

use crate::auth::{Capacidades, CapacidadesNoDisponible, UsuarioContexto};
use crate::comercio::Comercio;
use crate::config::Config;
use crate::contabilidad::Contabilidad;
use crate::errors::ApiError;
use crate::facturacion::{Facturacion, Ordenes, SinOrdenes};
use crate::finanzas::alertas::Alertas;
use crate::finanzas::kpis::{Kpis, SinVendedores, Vendedores};
use crate::finanzas::proyecciones::Proyecciones;
use crate::metrics::Metrics;
use crate::outbox::Outbox;
use crate::tributacion::Tributacion;
use std::sync::Arc;
use std::time::Instant;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub metrics: Arc<Metrics>,
    pub comercio: Arc<Comercio>,
    pub contabilidad: Arc<Contabilidad>,
    pub tributacion: Arc<Tributacion>,
    pub facturacion: Arc<Facturacion>,
    pub proyecciones: Arc<Proyecciones>,
    pub kpis: Arc<Kpis>,
    pub alertas: Arc<Alertas>,
    pub capacidades: Arc<dyn Capacidades>,
    pub inicio: Instant,
}

impl AppState {
    /// Estado sin infraestructura: pool diferido y clientes que no responden.
    /// Se usa en las pruebas HTTP de salud, docs y autenticación.
    pub fn nueva(config: Config, metrics: Metrics) -> Self {
        let db = Arc::new(crate::db::Db::perezoso("postgres://localhost/core_engine"));
        let outbox = Outbox::nueva(config.outbox_tabla.clone());
        let contabilidad = Arc::new(Contabilidad::nueva(Arc::clone(&db), outbox.clone()));
        let ordenes: Arc<dyn Ordenes> = Arc::new(SinOrdenes);
        let vendedores: Arc<dyn Vendedores> = Arc::new(SinVendedores);
        let proyecciones = Arc::new(Proyecciones::nueva(Arc::clone(&db)));
        let kpis = Arc::new(Kpis::nueva(Arc::clone(&db), vendedores));
        let tributacion = Arc::new(Tributacion::nueva(
            Arc::clone(&db),
            outbox.clone(),
            Arc::clone(&contabilidad),
        ));
        let facturacion = Arc::new(Facturacion::nueva(
            Arc::clone(&db),
            outbox.clone(),
            Arc::clone(&contabilidad),
            ordenes,
            false,
        ));
        let comercio = Arc::new(Comercio::nuevo(Arc::clone(&db)));
        let alertas = Arc::new(Alertas::nueva(
            Arc::clone(&kpis),
            Arc::clone(&proyecciones),
            Arc::clone(&db),
        ));
        Self {
            config: Arc::new(config),
            metrics: Arc::new(metrics),
            comercio,
            contabilidad,
            tributacion,
            facturacion,
            proyecciones,
            kpis,
            alertas,
            capacidades: Arc::new(CapacidadesNoDisponible),
            inicio: Instant::now(),
        }
    }

    pub fn con_capacidades(mut self, capacidades: Arc<dyn Capacidades>) -> Self {
        self.capacidades = capacidades;
        self
    }

    /// Reemplaza las dependencias de infraestructura construidas en `main`.
    #[allow(clippy::too_many_arguments)]
    pub fn con_servicios(
        mut self,
        comercio: Arc<Comercio>,
        contabilidad: Arc<Contabilidad>,
        tributacion: Arc<Tributacion>,
        facturacion: Arc<Facturacion>,
        proyecciones: Arc<Proyecciones>,
        kpis: Arc<Kpis>,
        alertas: Arc<Alertas>,
        metrics: Arc<Metrics>,
    ) -> Self {
        self.comercio = comercio;
        self.contabilidad = contabilidad;
        self.tributacion = tributacion;
        self.facturacion = facturacion;
        self.proyecciones = proyecciones;
        self.kpis = kpis;
        self.alertas = alertas;
        self.metrics = metrics;
        self
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
