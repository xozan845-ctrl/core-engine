//! KPIs del tablero financiero (tabla 8.8) alimentados por los eventos del bus:
//! las métricas se materializan en `finance.metricas_*` desde el consumidor.

use crate::db::Db;
use crate::errors::{error_db, ApiError};
use crate::money;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KpisFinancieros {
    pub periodo: String,
    pub gmv_cents: i64,
    pub gmv: String,
    pub pedidos_pagados: i64,
    pub pedidos_entregados: i64,
    pub ingresos_comisiones_cents: i64,
    pub ingresos_comisiones: String,
    pub devoluciones: i64,
    pub vendedores_activos: i64,
    pub pedidos_por_vendedor: f64,
    pub activacion_primeros_15_dias: Option<f64>,
    pub tasa_devoluciones: f64,
    pub churn_mensual: f64,
    pub conversion_carrito_checkout: Option<f64>,
    pub liquidacion_a_tiempo: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResumenVendedor {
    pub vendedor_id: String,
    pub periodo: String,
    pub ventas: i64,
    pub monto_cents: i64,
    pub comision_cents: i64,
    pub devoluciones: i64,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct VendedorRegistrado {
    pub id: String,
    pub creado_en: String,
}

/// Consulta del padrón de vendedores a identity (lectura interna).
#[async_trait]
pub trait Vendedores: Send + Sync {
    async fn registrados(&self) -> Option<Vec<VendedorRegistrado>>;
}

pub struct VendedoresHttp {
    url: String,
    clave: String,
    timeout: Duration,
}

impl VendedoresHttp {
    pub fn nueva(url: String, clave: Option<String>, timeout_ms: u64) -> Self {
        Self {
            url,
            clave: clave.unwrap_or_default(),
            timeout: Duration::from_millis(timeout_ms),
        }
    }
}

#[async_trait]
impl Vendedores for VendedoresHttp {
    async fn registrados(&self) -> Option<Vec<VendedorRegistrado>> {
        let cliente = reqwest::Client::builder()
            .timeout(self.timeout)
            .build()
            .ok()?;
        let respuesta = cliente
            .get(format!("{}/internal/vendedores", self.url))
            .header("x-internal-key", &self.clave)
            .send()
            .await
            .ok()?;
        if !respuesta.status().is_success() {
            return None;
        }
        let vendedores: Vec<VendedorRegistrado> = respuesta.json().await.ok()?;
        if vendedores.is_empty() {
            return None;
        }
        Some(vendedores)
    }
}

pub struct SinVendedores;

#[async_trait]
impl Vendedores for SinVendedores {
    async fn registrados(&self) -> Option<Vec<VendedorRegistrado>> {
        None
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Metrica {
    pub monto: i64,
    pub cantidad: i64,
}

pub struct Kpis {
    db: Arc<Db>,
    vendedores: Arc<dyn Vendedores>,
}

impl Kpis {
    pub fn nueva(db: Arc<Db>, vendedores: Arc<dyn Vendedores>) -> Self {
        Self { db, vendedores }
    }

    pub async fn kpis(&self, mes: Option<&str>) -> Result<KpisFinancieros, ApiError> {
        let periodo = mes
            .map(str::to_string)
            .unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
        let inicio_mes = format!("{}-01", periodo.get(0..7).unwrap_or("2026-01"));

        let periodos = self.leer_metricas_mensuales().await?;
        let gmv = periodos
            .get(&format!("{inicio_mes}|gmv"))
            .copied()
            .unwrap_or_default()
            .monto;
        let pedidos_pagados = periodos
            .get(&format!("{inicio_mes}|pedidos"))
            .copied()
            .unwrap_or_default()
            .cantidad;
        let entregados = periodos
            .get(&format!("{inicio_mes}|entregados"))
            .copied()
            .unwrap_or_default()
            .cantidad;
        let comisiones = periodos
            .get(&format!("{inicio_mes}|ingresos_comisiones"))
            .copied()
            .unwrap_or_default()
            .monto;
        let devoluciones = periodos
            .get(&format!("{inicio_mes}|devoluciones"))
            .copied()
            .unwrap_or_default()
            .cantidad;

        let vendedores_activos = self.vendedores_activos_30_dias().await?;
        let pedidos_por_vendedor = if vendedores_activos > 0 {
            pedidos_pagados as f64 / vendedores_activos as f64
        } else {
            0.0
        };
        let tasa_devoluciones = if entregados > 0 {
            devoluciones as f64 / entregados as f64
        } else {
            0.0
        };

        let (activacion, churn) = tokio::join!(self.activacion_15_dias(), self.churn_mensual());
        let conversion = Self::conversion_embudo(&periodos, &inicio_mes);
        let liquidacion = Self::liquidacion_a_tiempo(&periodos, &inicio_mes);

        Ok(KpisFinancieros {
            periodo: inicio_mes,
            gmv_cents: gmv,
            gmv: money::string(gmv),
            pedidos_pagados,
            pedidos_entregados: entregados,
            ingresos_comisiones_cents: comisiones,
            ingresos_comisiones: money::string(comisiones),
            devoluciones,
            vendedores_activos,
            pedidos_por_vendedor: ((pedidos_por_vendedor * 100.0).round()) / 100.0,
            activacion_primeros_15_dias: activacion,
            tasa_devoluciones: ((tasa_devoluciones * 10_000.0).round()) / 10_000.0,
            churn_mensual: ((churn * 10_000.0).round()) / 10_000.0,
            conversion_carrito_checkout: conversion,
            liquidacion_a_tiempo: liquidacion,
        })
    }

    /// Embudo carrito -> checkout (tabla 8.8); `null` sin datos.
    pub fn conversion_embudo(periodos: &HashMap<String, Metrica>, inicio_mes: &str) -> Option<f64> {
        let carritos = periodos
            .get(&format!("{inicio_mes}|carritos_iniciados"))
            .copied()
            .unwrap_or_default()
            .cantidad;
        let checkouts = periodos
            .get(&format!("{inicio_mes}|checkouts_completados"))
            .copied()
            .unwrap_or_default()
            .cantidad;
        if carritos <= 0 || checkouts <= 0 {
            return None;
        }
        Some(((checkouts as f64 / carritos as f64) * 10_000.0).round() / 10_000.0)
    }

    /// Liquidación a tiempo (RN-07); `null` sin datos.
    pub fn liquidacion_a_tiempo(
        periodos: &HashMap<String, Metrica>,
        inicio_mes: &str,
    ) -> Option<f64> {
        let a_tiempo = periodos
            .get(&format!("{inicio_mes}|liquidaciones_a_tiempo"))
            .copied()
            .unwrap_or_default()
            .cantidad;
        let esperados = periodos
            .get(&format!("{inicio_mes}|cortes_esperados"))
            .copied()
            .unwrap_or_default()
            .cantidad;
        if esperados <= 0 {
            return None;
        }
        Some(((a_tiempo as f64 / esperados as f64) * 10_000.0).round() / 10_000.0)
    }

    async fn leer_metricas_mensuales(&self) -> Result<HashMap<String, Metrica>, ApiError> {
        let filas = sqlx::query(
            "SELECT TO_CHAR(periodo, 'YYYY-MM-DD') AS periodo, tipo, monto_cents, cantidad
             FROM finance.metricas_mensuales",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        let mut mapa = HashMap::new();
        for fila in filas {
            use sqlx::Row;
            let periodo: String = fila.try_get("periodo").unwrap_or_default();
            let tipo: String = fila.try_get("tipo").unwrap_or_default();
            mapa.insert(
                format!("{periodo}|{tipo}"),
                Metrica {
                    monto: fila.try_get::<i32, _>("monto_cents").unwrap_or(0) as i64,
                    cantidad: fila.try_get::<i32, _>("cantidad").unwrap_or(0) as i64,
                },
            );
        }
        Ok(mapa)
    }

    /// Vendedores con al menos 1 venta en los últimos 30 días (tabla 8.8).
    async fn vendedores_activos_30_dias(&self) -> Result<i64, ApiError> {
        let total: Option<i32> = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT vendedor_id)::int AS n
             FROM finance.metricas_vendedores
             WHERE ultima_venta_en >= NOW() - INTERVAL '30 days'",
        )
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(total.unwrap_or(0) as i64)
    }

    /// 1.ª venta en <= 15 días desde el registro / vendedores registrados.
    async fn activacion_15_dias(&self) -> Option<f64> {
        let vendedores = self.vendedores.registrados().await?;
        let filas = sqlx::query(
            "SELECT vendedor_id, ventas, primera_venta_en
             FROM finance.metricas_vendedores",
        )
        .fetch_all(self.db.pool())
        .await
        .ok()?;
        let mut periodos: HashMap<String, (i64, Option<DateTime<Utc>>)> = HashMap::new();
        for fila in filas {
            use sqlx::Row;
            let vendedor_id: String = fila.try_get("vendedor_id").ok()?;
            periodos.insert(
                vendedor_id,
                (
                    fila.try_get::<i32, _>("ventas").unwrap_or(0) as i64,
                    fila.try_get::<Option<DateTime<Utc>>, _>("primera_venta_en")
                        .ok()
                        .flatten(),
                ),
            );
        }
        let mut registrados = 0i64;
        let mut activados = 0i64;
        for vendedor in vendedores {
            let Some((ventas, primera_venta)) = periodos.get(&vendedor.id) else {
                continue;
            };
            if *ventas == 0 {
                continue;
            }
            let Some(primera_venta) = primera_venta else {
                continue;
            };
            let Ok(registro) = DateTime::parse_from_rfc3339(&vendedor.creado_en) else {
                continue;
            };
            registrados += 1;
            if (*primera_venta - registro.with_timezone(&Utc)).num_days() <= 15 {
                activados += 1;
            }
        }
        if registrados == 0 {
            None
        } else {
            Some(activados as f64 / registrados as f64)
        }
    }

    /// Vendedores inactivos / vendedores activos del periodo anterior (8.8).
    async fn churn_mensual(&self) -> f64 {
        let previos: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT vendedor_id FROM finance.metricas_vendedores
             WHERE ultima_venta_en >= NOW() - INTERVAL '60 days'
               AND ultima_venta_en < NOW() - INTERVAL '30 days'",
        )
        .fetch_all(self.db.pool())
        .await
        .unwrap_or_default();
        if previos.is_empty() {
            return 0.0;
        }
        let activos: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT vendedor_id FROM finance.metricas_vendedores
             WHERE ultima_venta_en >= NOW() - INTERVAL '30 days'",
        )
        .fetch_all(self.db.pool())
        .await
        .unwrap_or_default();
        let activos: HashSet<String> = activos.into_iter().collect();
        let inactivos = previos
            .iter()
            .filter(|vendedor| !activos.contains(*vendedor))
            .count();
        inactivos as f64 / previos.len() as f64
    }

    /// Resumen de negocios de un vendedor para su declaración (cap. 4.4).
    pub async fn resumen_del_vendedor(
        &self,
        db: &Db,
        vendedor_id: &str,
        mes: Option<&str>,
    ) -> Result<Option<ResumenVendedor>, ApiError> {
        let periodo = mes
            .map(str::to_string)
            .unwrap_or_else(|| Utc::now().format("%Y-%m").to_string());
        let uuid = uuid::Uuid::parse_str(vendedor_id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El vendedor debe ser un UUID."))?;
        let fila = sqlx::query(
            "SELECT vendedor_id,
                    SUM(ventas)::int AS ventas,
                    SUM(monto_cents)::int AS monto_cents,
                    SUM(comision_cents)::int AS comision_cents,
                    SUM(devoluciones)::int AS devoluciones
             FROM finance.metricas_vendedores
             WHERE vendedor_id = $1 AND TO_CHAR(periodo, 'YYYY-MM') = $2
             GROUP BY vendedor_id",
        )
        .bind(uuid)
        .bind(&periodo)
        .fetch_optional(db.pool())
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Ok(None);
        };
        use sqlx::Row;
        let ventas = fila.try_get::<i32, _>("ventas").unwrap_or(0) as i64;
        if ventas == 0 {
            return Ok(None);
        }
        Ok(Some(ResumenVendedor {
            vendedor_id: vendedor_id.to_string(),
            periodo,
            ventas,
            monto_cents: fila.try_get::<i32, _>("monto_cents").unwrap_or(0) as i64,
            comision_cents: fila.try_get::<i32, _>("comision_cents").unwrap_or(0) as i64,
            devoluciones: fila.try_get::<i32, _>("devoluciones").unwrap_or(0) as i64,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_calcular_el_embudo_solo_con_datos() {
        let mut periodos = HashMap::new();
        assert_eq!(Kpis::conversion_embudo(&periodos, "2026-11-01"), None);
        periodos.insert(
            "2026-11-01|carritos_iniciados".into(),
            Metrica {
                monto: 0,
                cantidad: 40,
            },
        );
        periodos.insert(
            "2026-11-01|checkouts_completados".into(),
            Metrica {
                monto: 0,
                cantidad: 10,
            },
        );
        assert_eq!(Kpis::conversion_embudo(&periodos, "2026-11-01"), Some(0.25));
    }

    #[test]
    fn debe_calcular_la_liquidacion_a_tiempo() {
        let mut periodos = HashMap::new();
        assert_eq!(Kpis::liquidacion_a_tiempo(&periodos, "2026-11-01"), None);
        periodos.insert(
            "2026-11-01|cortes_esperados".into(),
            Metrica {
                monto: 0,
                cantidad: 4,
            },
        );
        periodos.insert(
            "2026-11-01|liquidaciones_a_tiempo".into(),
            Metrica {
                monto: 0,
                cantidad: 3,
            },
        );
        assert_eq!(
            Kpis::liquidacion_a_tiempo(&periodos, "2026-11-01"),
            Some(0.75)
        );
    }
}
