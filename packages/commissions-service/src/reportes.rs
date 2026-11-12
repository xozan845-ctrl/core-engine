//! KPIs de ventas e inventario (H-06 / GET /api/v2/admin/reportes).
//!
//! Las métricas internas derivan de los eventos de negocio; las externas se
//! consultan a catálogo e identidad y degradan a cero si no responden.

use crate::clients::{InventarioResumen, Kpis};
use crate::db::{error_db, Db};
use crate::errors::ApiError;
use crate::money;
use async_trait::async_trait;
use serde::Serialize;
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ReporteKpi {
    pub gmv_mes_cents: i64,
    pub gmv_mes: String,
    pub pedidos_mes: i64,
    pub comisiones_mes_cents: i64,
    pub comisiones_mes: String,
    pub vendedores_activos: i64,
    pub productos: i64,
    pub stock_total: i64,
    pub agotados: i64,
    pub devoluciones_mes: i64,
}

#[allow(clippy::too_many_arguments)]
pub fn construir_reporte(
    gmv: i64,
    comisiones: i64,
    pedidos: i64,
    devoluciones: i64,
    inventario: InventarioResumen,
    vendedores: i64,
) -> ReporteKpi {
    ReporteKpi {
        gmv_mes_cents: gmv,
        gmv_mes: money::string(gmv),
        pedidos_mes: pedidos,
        comisiones_mes_cents: comisiones,
        comisiones_mes: money::string(comisiones),
        vendedores_activos: vendedores,
        productos: inventario.productos,
        stock_total: inventario.stock_total,
        agotados: inventario.agotados,
        devoluciones_mes: devoluciones,
    }
}

#[async_trait]
pub trait ReportesPort: Send + Sync {
    async fn kpis(&self) -> Result<ReporteKpi, ApiError>;
}

pub struct Reportes {
    db: Arc<Db>,
    externos: Arc<dyn Kpis>,
}

impl Reportes {
    pub fn nueva(db: Arc<Db>, externos: Arc<dyn Kpis>) -> Self {
        Self { db, externos }
    }

    async fn escalar(&self, sql: &str, columna: &str) -> Result<i64, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(sql)
            .fetch_one(&mut *tx)
            .await
            .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        Ok(i64::from(
            fila.try_get::<i32, _>(columna).map_err(error_db)?,
        ))
    }

    async fn gmv(&self) -> Result<i64, ApiError> {
        self.escalar(
            "SELECT COALESCE(SUM(venta_cents), 0)::int AS total
             FROM commissions.comisiones WHERE creado_en >= date_trunc('month', NOW())",
            "total",
        )
        .await
    }

    async fn pedidos(&self) -> Result<i64, ApiError> {
        self.escalar(
            "SELECT COUNT(*)::int AS n FROM commissions.pagos
             WHERE creado_en >= date_trunc('month', NOW())",
            "n",
        )
        .await
    }

    async fn comisiones(&self) -> Result<i64, ApiError> {
        self.escalar(
            "SELECT COALESCE(SUM(comision_cents), 0)::int AS total
             FROM commissions.comisiones WHERE creado_en >= date_trunc('month', NOW())",
            "total",
        )
        .await
    }

    async fn devoluciones(&self) -> Result<i64, ApiError> {
        self.escalar(
            "SELECT COUNT(*)::int AS n FROM commissions.compensaciones_devoluciones
             WHERE creado_en >= date_trunc('month', NOW())",
            "n",
        )
        .await
    }
}

#[async_trait]
impl ReportesPort for Reportes {
    async fn kpis(&self) -> Result<ReporteKpi, ApiError> {
        let (gmv, comisiones, devoluciones, inventario, vendedores, pedidos) = tokio::join!(
            self.gmv(),
            self.comisiones(),
            self.devoluciones(),
            self.externos.inventario(),
            self.externos.vendedores(),
            self.pedidos(),
        );
        Ok(construir_reporte(
            gmv?,
            comisiones?,
            pedidos?,
            devoluciones?,
            inventario,
            vendedores,
        ))
    }
}

/// Implementación para tests: el repositorio no está disponible.
pub struct ReportesNoDisponible;

#[async_trait]
impl ReportesPort for ReportesNoDisponible {
    async fn kpis(&self) -> Result<ReporteKpi, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_construir_los_kpis_con_montos_y_conteos() {
        let reporte = construir_reporte(
            12_345,
            2_345,
            4,
            2,
            InventarioResumen {
                productos: 12,
                stock_total: 80,
                agotados: 3,
            },
            5,
        );
        assert_eq!(reporte.gmv_mes_cents, 12_345);
        assert_eq!(reporte.gmv_mes, "123.45");
        assert_eq!(reporte.comisiones_mes, "23.45");
        assert_eq!(reporte.pedidos_mes, 4);
        assert_eq!(reporte.vendedores_activos, 5);
        assert_eq!(reporte.productos, 12);
        assert_eq!(reporte.stock_total, 80);
        assert_eq!(reporte.agotados, 3);
        assert_eq!(reporte.devoluciones_mes, 2);
    }

    #[test]
    fn debe_formatear_cero_sin_errores() {
        let reporte = construir_reporte(0, 0, 0, 0, InventarioResumen::default(), 0);
        assert_eq!(reporte.gmv_mes, "0.00");
        assert_eq!(reporte.comisiones_mes, "0.00");
    }
}
