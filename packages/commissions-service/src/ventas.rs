//! Libro del vendedor y de la empresa (H-04, ADR-25).
//!
//! Lecturas contra el esquema propio (CQRS: proyección de comisiones) con los
//! mismos montos formateados en córdobas con dos decimales.

use crate::comisiones::{Comision, ComisionesPort};
use crate::db::{error_db, fecha, parsear_uuid, timestamp, uuid_texto, Db};
use crate::errors::ApiError;
use crate::liquidaciones::{Liquidacion, LiquidacionEmpresa};
use crate::money;
use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VentaVendedor {
    pub order_id: String,
    pub venta: String,
    pub comision: String,
    pub monto_vendedor: String,
    pub estado: String,
    pub fecha: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EstadoCuenta {
    pub vendedor_id: String,
    pub ventas: Vec<VentaVendedor>,
    pub comisiones_pendientes: String,
    pub liquidaciones: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VentaEmpresa {
    pub order_id: String,
    pub venta: String,
    pub comision: String,
    pub monto_empresario: String,
    pub estado: String,
    pub fecha: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EstadoCuentaEmpresa {
    pub empresa_id: String,
    pub ventas: Vec<VentaEmpresa>,
    pub comisiones_pendientes: String,
    pub liquidaciones: Vec<Value>,
}

pub fn liquidacion_con_monto(liquidacion: Liquidacion) -> Result<Value, ApiError> {
    let monto = money::string(liquidacion.monto_cents);
    let mut valor = serde_json::to_value(liquidacion).map_err(|_| ApiError::interno())?;
    if let Some(objeto) = valor.as_object_mut() {
        objeto.insert("monto".to_string(), json!(monto));
    }
    Ok(valor)
}

pub fn liquidacion_empresa_con_monto(liquidacion: LiquidacionEmpresa) -> Result<Value, ApiError> {
    let monto = money::string(liquidacion.monto_cents);
    let mut valor = serde_json::to_value(liquidacion).map_err(|_| ApiError::interno())?;
    if let Some(objeto) = valor.as_object_mut() {
        objeto.insert("monto".to_string(), json!(monto));
    }
    Ok(valor)
}

fn liquidacion_desde_fila(fila: &PgRow) -> Result<Liquidacion, sqlx::Error> {
    Ok(Liquidacion {
        id: uuid_texto(fila, "id")?,
        vendedor_id: uuid_texto(fila, "vendedor_id")?,
        periodo_inicio: fecha(fila, "periodo_inicio")?,
        periodo_fin: fecha(fila, "periodo_fin")?,
        monto_cents: i64::from(fila.try_get::<i32, _>("monto_cents")?),
        estado: fila.try_get::<String, _>("estado")?,
        creado_en: timestamp(fila, "creado_en")?,
    })
}

fn liquidacion_empresa_desde_fila(fila: &PgRow) -> Result<LiquidacionEmpresa, sqlx::Error> {
    Ok(LiquidacionEmpresa {
        id: uuid_texto(fila, "id")?,
        empresa_id: uuid_texto(fila, "empresa_id")?,
        periodo_inicio: fecha(fila, "periodo_inicio")?,
        periodo_fin: fecha(fila, "periodo_fin")?,
        monto_cents: i64::from(fila.try_get::<i32, _>("monto_cents")?),
        estado: fila.try_get::<String, _>("estado")?,
        creado_en: timestamp(fila, "creado_en")?,
    })
}

#[async_trait]
pub trait VentasPort: Send + Sync {
    async fn estado_cuenta(&self, vendedor_id: &str) -> Result<EstadoCuenta, ApiError>;
    async fn estado_cuenta_empresa(
        &self,
        empresa_id: &str,
    ) -> Result<EstadoCuentaEmpresa, ApiError>;
}

pub struct Ventas {
    db: Arc<Db>,
    comisiones: Arc<dyn ComisionesPort>,
}

impl Ventas {
    pub fn nueva(db: Arc<Db>, comisiones: Arc<dyn ComisionesPort>) -> Self {
        Self { db, comisiones }
    }
}

fn venta_vendedor(comision: &Comision) -> VentaVendedor {
    VentaVendedor {
        order_id: comision.order_id.clone(),
        venta: money::string(comision.venta_cents),
        comision: money::string(comision.comision_cents),
        monto_vendedor: money::string(comision.monto_vendedor_cents),
        estado: comision.estado.clone(),
        fecha: comision.creado_en.clone(),
    }
}

fn venta_empresa(comision: &Comision) -> VentaEmpresa {
    VentaEmpresa {
        order_id: comision.order_id.clone(),
        venta: money::string(comision.venta_cents),
        comision: money::string(comision.comision_cents),
        monto_empresario: money::string(comision.monto_empresario_cents.unwrap_or(0)),
        estado: comision.estado.clone(),
        fecha: comision.creado_en.clone(),
    }
}

#[async_trait]
impl VentasPort for Ventas {
    async fn estado_cuenta(&self, vendedor_id: &str) -> Result<EstadoCuenta, ApiError> {
        let comisiones = self.comisiones.comisiones_de(vendedor_id).await?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en
             FROM commissions.liquidaciones WHERE vendedor_id = $1
             ORDER BY periodo_inicio DESC LIMIT 50",
        )
        .bind(parsear_uuid(vendedor_id)?)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        let liquidaciones = filas
            .iter()
            .map(liquidacion_desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)?
            .into_iter()
            .map(liquidacion_con_monto)
            .collect::<Result<Vec<_>, _>>()?;
        let pendientes = comisiones
            .iter()
            .filter(|comision| comision.estado == "acreditada")
            .map(|comision| comision.monto_vendedor_cents)
            .sum::<i64>();
        Ok(EstadoCuenta {
            vendedor_id: vendedor_id.to_string(),
            ventas: comisiones.iter().map(venta_vendedor).collect(),
            comisiones_pendientes: money::string(pendientes),
            liquidaciones,
        })
    }

    async fn estado_cuenta_empresa(
        &self,
        empresa_id: &str,
    ) -> Result<EstadoCuentaEmpresa, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let comisiones = sqlx::query(
            "SELECT id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents,
                    comision_cents, monto_vendedor_cents, monto_empresario_cents,
                    participacion_porcentaje, estado, creado_en
             FROM commissions.comisiones WHERE empresa_id = $1 ORDER BY creado_en DESC LIMIT 200",
        )
        .bind(parsear_uuid(empresa_id)?)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        let liquidaciones_filas = sqlx::query(
            "SELECT id, empresa_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en
             FROM commissions.liquidaciones_empresas WHERE empresa_id = $1
             ORDER BY periodo_inicio DESC LIMIT 50",
        )
        .bind(parsear_uuid(empresa_id)?)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        let comisiones = comisiones
            .iter()
            .map(Comision::desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)?;
        let liquidaciones = liquidaciones_filas
            .iter()
            .map(liquidacion_empresa_desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)?
            .into_iter()
            .map(liquidacion_empresa_con_monto)
            .collect::<Result<Vec<_>, _>>()?;
        let pendientes = comisiones
            .iter()
            .filter(|comision| comision.estado == "acreditada")
            .map(|comision| comision.monto_empresario_cents.unwrap_or(0))
            .sum::<i64>();
        Ok(EstadoCuentaEmpresa {
            empresa_id: empresa_id.to_string(),
            ventas: comisiones.iter().map(venta_empresa).collect(),
            comisiones_pendientes: money::string(pendientes),
            liquidaciones,
        })
    }
}

/// Implementación para tests: el repositorio no está disponible.
pub struct VentasNoDisponible;

#[async_trait]
impl VentasPort for VentasNoDisponible {
    async fn estado_cuenta(&self, _vendedor_id: &str) -> Result<EstadoCuenta, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }

    async fn estado_cuenta_empresa(
        &self,
        _empresa_id: &str,
    ) -> Result<EstadoCuentaEmpresa, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comision(
        estado: &str,
        venta: i64,
        comision: i64,
        vendedor: i64,
        empresario: i64,
    ) -> Comision {
        Comision {
            id: "c-1".into(),
            order_id: "o-1".into(),
            linea_clave: Some("l-1".into()),
            empresa_id: Some("e-1".into()),
            tienda_id: Some("t-1".into()),
            vendedor_id: "v-1".into(),
            venta_cents: venta,
            comision_cents: comision,
            monto_vendedor_cents: vendedor,
            monto_empresario_cents: Some(empresario),
            participacion_porcentaje: Some(60),
            estado: estado.into(),
            creado_en: "2026-09-10T00:00:00.000Z".into(),
        }
    }

    #[test]
    fn debe_formatear_la_venta_del_vendedor() {
        let venta = venta_vendedor(&comision("acreditada", 115_000, 13_800, 101_200, 0));
        assert_eq!(
            venta,
            VentaVendedor {
                order_id: "o-1".into(),
                venta: "1150.00".into(),
                comision: "138.00".into(),
                monto_vendedor: "1012.00".into(),
                estado: "acreditada".into(),
                fecha: "2026-09-10T00:00:00.000Z".into(),
            }
        );
    }

    #[test]
    fn debe_formatear_la_venta_de_la_empresa() {
        let venta = venta_empresa(&comision("liquidada", 50_000, 6_000, 0, 5_400));
        assert_eq!(venta.venta, "500.00");
        assert_eq!(venta.comision, "60.00");
        assert_eq!(venta.monto_empresario, "54.00");
        assert_eq!(venta.estado, "liquidada");
    }

    #[test]
    fn debe_agregar_el_monto_formateado_al_final() {
        let valor = liquidacion_con_monto(Liquidacion {
            id: "l-1".into(),
            vendedor_id: "v-1".into(),
            periodo_inicio: "2026-09-01T06:00:00.000Z".into(),
            periodo_fin: "2026-09-15T06:00:00.000Z".into(),
            monto_cents: 18_000,
            estado: "aprobada".into(),
            creado_en: "2026-09-16T00:00:00.000Z".into(),
        })
        .expect("valor");
        assert_eq!(valor["monto"], "180.00");
        let claves: Vec<&String> = valor.as_object().expect("objeto").keys().collect();
        assert_eq!(claves.last().map(|clave| clave.as_str()), Some("monto"));
    }
}
