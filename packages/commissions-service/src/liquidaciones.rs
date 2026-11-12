//! Liquidaciones quincenales (RN-07) y consultas del libro.
//!
//! Réplica de `liquidaciones.service.ts`: corte los días 1 y 15, consolidación
//! por vendedor y empresa, compensaciones de devoluciones y pago de la
//! liquidación.

use crate::campos;
use crate::db::{centavos_i32, error_db, fecha, parsear_uuid, timestamp, uuid_texto, Db};
use crate::errors::ApiError;
use async_trait::async_trait;
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Periodo {
    pub inicio: String,
    pub fin: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Liquidacion {
    pub id: String,
    pub vendedor_id: String,
    pub periodo_inicio: String,
    pub periodo_fin: String,
    pub monto_cents: i64,
    pub estado: String,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiquidacionEmpresa {
    pub id: String,
    pub empresa_id: String,
    pub periodo_inicio: String,
    pub periodo_fin: String,
    pub monto_cents: i64,
    pub estado: String,
    pub creado_en: String,
}

fn formato(fecha: NaiveDate) -> String {
    fecha.format("%Y-%m-%d").to_string()
}

fn ultimo_dia(anio: i32, mes: u32) -> NaiveDate {
    let (siguiente_anio, siguiente_mes) = if mes == 12 {
        (anio + 1, 1)
    } else {
        (anio, mes + 1)
    };
    NaiveDate::from_ymd_opt(siguiente_anio, siguiente_mes, 1)
        .expect("primer dia del mes siguiente")
        .pred_opt()
        .expect("ultimo dia del mes")
}

/// Periodo quincenal al que pertenece una fecha (RN-07: cortes días 1 y 15).
pub fn periodo_de(fecha: NaiveDate) -> Periodo {
    let (anio, mes, dia) = (fecha.year(), fecha.month(), fecha.day());
    if dia <= 15 {
        Periodo {
            inicio: formato(NaiveDate::from_ymd_opt(anio, mes, 1).expect("inicio")),
            fin: formato(NaiveDate::from_ymd_opt(anio, mes, 15).expect("fin")),
        }
    } else {
        Periodo {
            inicio: formato(NaiveDate::from_ymd_opt(anio, mes, 16).expect("inicio")),
            fin: formato(ultimo_dia(anio, mes)),
        }
    }
}

/// Periodo que cierra cada corte: el día 1 cierra el tramo 16..fin del mes
/// anterior; el día 15 cierra la primera quincena; el 16+ cierra 16..fin.
pub fn periodo_a_cerrar(fecha: NaiveDate) -> Periodo {
    let (anio, mes, dia) = (fecha.year(), fecha.month(), fecha.day());
    if dia == 1 {
        let (previo_anio, previo_mes) = if mes == 1 {
            (anio - 1, 12)
        } else {
            (anio, mes - 1)
        };
        Periodo {
            inicio: formato(NaiveDate::from_ymd_opt(previo_anio, previo_mes, 16).expect("inicio")),
            fin: formato(ultimo_dia(previo_anio, previo_mes)),
        }
    } else if dia <= 15 {
        Periodo {
            inicio: formato(NaiveDate::from_ymd_opt(anio, mes, 1).expect("inicio")),
            fin: formato(NaiveDate::from_ymd_opt(anio, mes, 15).expect("fin")),
        }
    } else {
        Periodo {
            inicio: formato(NaiveDate::from_ymd_opt(anio, mes, 16).expect("inicio")),
            fin: formato(ultimo_dia(anio, mes)),
        }
    }
}

fn sumar(mapa: &mut Vec<(String, i64)>, clave: String, monto: i64) {
    if let Some(fila) = mapa.iter_mut().find(|(actual, _)| actual == &clave) {
        fila.1 += monto;
    } else {
        mapa.push((clave, monto));
    }
}

impl Liquidacion {
    fn desde_fila(fila: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: uuid_texto(fila, "id")?,
            vendedor_id: uuid_texto(fila, "vendedor_id")?,
            periodo_inicio: fecha(fila, "periodo_inicio")?,
            periodo_fin: fecha(fila, "periodo_fin")?,
            monto_cents: i64::from(fila.try_get::<i32, _>("monto_cents")?),
            estado: fila.try_get::<String, _>("estado")?,
            creado_en: timestamp(fila, "creado_en")?,
        })
    }
}

impl LiquidacionEmpresa {
    fn desde_fila(fila: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: uuid_texto(fila, "id")?,
            empresa_id: uuid_texto(fila, "empresa_id")?,
            periodo_inicio: fecha(fila, "periodo_inicio")?,
            periodo_fin: fecha(fila, "periodo_fin")?,
            monto_cents: i64::from(fila.try_get::<i32, _>("monto_cents")?),
            estado: fila.try_get::<String, _>("estado")?,
            creado_en: timestamp(fila, "creado_en")?,
        })
    }
}

#[async_trait]
pub trait LiquidacionesPort: Send + Sync {
    async fn cerrar_periodo(&self, periodo: Periodo) -> Result<Vec<Liquidacion>, ApiError>;
    async fn cerrar_periodo_anterior(&self, fecha: NaiveDate)
        -> Result<Vec<Liquidacion>, ApiError>;
    async fn de_vendedor(&self, vendedor_id: &str) -> Result<Vec<Liquidacion>, ApiError>;
    async fn de_empresa(&self, empresa_id: &str) -> Result<Vec<LiquidacionEmpresa>, ApiError>;
    async fn marcar_pagada(&self, liquidacion_id: &str) -> Result<Option<Liquidacion>, ApiError>;
}

pub struct Liquidaciones {
    db: Arc<Db>,
}

impl Liquidaciones {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    async fn consultar_liquidaciones(
        &self,
        sql: &str,
        id: &str,
    ) -> Result<Vec<Liquidacion>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(sql)
            .bind(parsear_uuid(id)?)
            .fetch_all(&mut *tx)
            .await
            .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas
            .iter()
            .map(Liquidacion::desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)
    }
}

#[async_trait]
impl LiquidacionesPort for Liquidaciones {
    /// Cierra un periodo: consolida comisiones acreditadas y compensaciones.
    async fn cerrar_periodo(&self, periodo: Periodo) -> Result<Vec<Liquidacion>, ApiError> {
        let Periodo { inicio, fin } = periodo;
        let inicio_fecha =
            NaiveDate::parse_from_str(&inicio, "%Y-%m-%d").map_err(|_| ApiError::interno())?;
        let fin_fecha =
            NaiveDate::parse_from_str(&fin, "%Y-%m-%d").map_err(|_| ApiError::interno())?;

        // 1. marcar como liquidadas las comisiones acreditadas del periodo
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "UPDATE commissions.comisiones SET estado = 'liquidada'
             WHERE estado = 'acreditada' AND creado_en >= $1::date
               AND creado_en <= $2::date + INTERVAL '1 day' - INTERVAL '1 second'
             RETURNING id, vendedor_id, empresa_id, monto_vendedor_cents, monto_empresario_cents",
        )
        .bind(inicio_fecha)
        .bind(fin_fecha)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        if filas.is_empty() {
            return Ok(Vec::new());
        }

        // 2. consolidar por vendedor y empresa
        let mut por_vendedor: Vec<(String, i64)> = Vec::new();
        let mut por_empresa: Vec<(String, i64)> = Vec::new();
        for fila in filas {
            let vendedor = uuid_texto(&fila, "vendedor_id").map_err(error_db)?;
            let empresa = fila
                .try_get::<Option<uuid::Uuid>, _>("empresa_id")
                .map_err(error_db)?
                .map(|valor| valor.to_string())
                .unwrap_or_default();
            let monto_vendedor = i64::from(
                fila.try_get::<i32, _>("monto_vendedor_cents")
                    .map_err(error_db)?,
            );
            let monto_empresario = i64::from(
                fila.try_get::<Option<i32>, _>("monto_empresario_cents")
                    .map_err(error_db)?
                    .unwrap_or(0),
            );
            sumar(&mut por_vendedor, vendedor, monto_vendedor);
            if !empresa.is_empty() {
                sumar(&mut por_empresa, empresa, monto_empresario);
            }
        }

        // 3. compensaciones del periodo (RN-06: devoluciones)
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let compensaciones = sqlx::query(
            "SELECT vendedor_id, COALESCE(SUM(monto_cents), 0)::int AS total
             FROM commissions.compensaciones_devoluciones
             WHERE creado_en >= $1::date
               AND creado_en <= $2::date + INTERVAL '1 day' - INTERVAL '1 second'
             GROUP BY vendedor_id",
        )
        .bind(inicio_fecha)
        .bind(fin_fecha)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        let compensaciones_empresas = sqlx::query(
            "SELECT empresa_id, COALESCE(SUM(monto_empresario_cents), 0)::int AS total
             FROM commissions.compensaciones_devoluciones
             WHERE creado_en >= $1::date
               AND creado_en <= $2::date + INTERVAL '1 day' - INTERVAL '1 second'
             GROUP BY empresa_id",
        )
        .bind(inicio_fecha)
        .bind(fin_fecha)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        for fila in compensaciones {
            let vendedor = uuid_texto(&fila, "vendedor_id").map_err(error_db)?;
            let total = i64::from(fila.try_get::<i32, _>("total").map_err(error_db)?);
            sumar(&mut por_vendedor, vendedor, total);
        }
        for fila in compensaciones_empresas {
            let empresa = uuid_texto(&fila, "empresa_id").map_err(error_db)?;
            let total = i64::from(fila.try_get::<i32, _>("total").map_err(error_db)?);
            sumar(&mut por_empresa, empresa, total);
        }

        // 4. crear liquidaciones aprobadas
        let mut liquidaciones = Vec::new();
        for (vendedor_id, monto) in por_vendedor {
            if monto == 0 {
                continue;
            }
            let mut tx = self.db.transaccion().await.map_err(error_db)?;
            let fila = sqlx::query(
                "INSERT INTO commissions.liquidaciones
                    (id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado)
                 VALUES (gen_random_uuid(), $1, $2, $3, $4, 'aprobada')
                 ON CONFLICT (vendedor_id, periodo_inicio) DO NOTHING
                 RETURNING id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en",
            )
            .bind(parsear_uuid(&vendedor_id)?)
            .bind(inicio_fecha)
            .bind(fin_fecha)
            .bind(centavos_i32(monto)?)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?;
            tx.commit().await.map_err(error_db)?;
            if let Some(fila) = fila {
                liquidaciones.push(Liquidacion::desde_fila(&fila).map_err(error_db)?);
            }
        }
        for (empresa_id, monto) in por_empresa {
            if monto == 0 {
                continue;
            }
            let mut tx = self.db.transaccion().await.map_err(error_db)?;
            sqlx::query(
                "INSERT INTO commissions.liquidaciones_empresas
                    (id, empresa_id, periodo_inicio, periodo_fin, monto_cents, estado)
                 VALUES (gen_random_uuid(), $1,$2,$3,$4,'aprobada')
                 ON CONFLICT (empresa_id, periodo_inicio) DO NOTHING",
            )
            .bind(parsear_uuid(&empresa_id)?)
            .bind(inicio_fecha)
            .bind(fin_fecha)
            .bind(centavos_i32(monto)?)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
            tx.commit().await.map_err(error_db)?;
        }

        crate::logs::info(
            "commissions.liquidacion",
            campos! {
                "msg": "Corte quincenal ejecutado",
                "periodo": format!("{inicio}..{fin}"),
                "vendedores": liquidaciones.len(),
            },
        );
        Ok(liquidaciones)
    }

    async fn cerrar_periodo_anterior(
        &self,
        fecha: NaiveDate,
    ) -> Result<Vec<Liquidacion>, ApiError> {
        self.cerrar_periodo(periodo_a_cerrar(fecha)).await
    }

    async fn de_vendedor(&self, vendedor_id: &str) -> Result<Vec<Liquidacion>, ApiError> {
        self.consultar_liquidaciones(
            "SELECT id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en
             FROM commissions.liquidaciones WHERE vendedor_id = $1
             ORDER BY periodo_inicio DESC LIMIT 100",
            vendedor_id,
        )
        .await
    }

    async fn de_empresa(&self, empresa_id: &str) -> Result<Vec<LiquidacionEmpresa>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT id, empresa_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en
             FROM commissions.liquidaciones_empresas WHERE empresa_id = $1
             ORDER BY periodo_inicio DESC LIMIT 100",
        )
        .bind(parsear_uuid(empresa_id)?)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas
            .iter()
            .map(LiquidacionEmpresa::desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)
    }

    async fn marcar_pagada(&self, liquidacion_id: &str) -> Result<Option<Liquidacion>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "UPDATE commissions.liquidaciones SET estado = 'pagada'
             WHERE id = $1
             RETURNING id, vendedor_id, periodo_inicio, periodo_fin, monto_cents, estado, creado_en",
        )
        .bind(parsear_uuid(liquidacion_id)?)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila.map(|fila| Liquidacion::desde_fila(&fila))
            .transpose()
            .map_err(error_db)
    }
}

/// Implementación para tests: el repositorio no está disponible.
pub struct LiquidacionesNoDisponible;

#[async_trait]
impl LiquidacionesPort for LiquidacionesNoDisponible {
    async fn cerrar_periodo(&self, _periodo: Periodo) -> Result<Vec<Liquidacion>, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }

    async fn cerrar_periodo_anterior(
        &self,
        _fecha: NaiveDate,
    ) -> Result<Vec<Liquidacion>, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }

    async fn de_vendedor(&self, _vendedor_id: &str) -> Result<Vec<Liquidacion>, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }

    async fn de_empresa(&self, _empresa_id: &str) -> Result<Vec<LiquidacionEmpresa>, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }

    async fn marcar_pagada(&self, _id: &str) -> Result<Option<Liquidacion>, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }
}

#[allow(dead_code)]
fn _tipos(_tx: &mut Transaction<'_, Postgres>) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).expect("fecha")
    }

    #[test]
    fn debe_calcular_la_quincena_de_una_fecha() {
        assert_eq!(
            periodo_de(fecha(2026, 1, 5)),
            Periodo {
                inicio: "2026-01-01".into(),
                fin: "2026-01-15".into()
            }
        );
        assert_eq!(
            periodo_de(fecha(2026, 9, 9)),
            Periodo {
                inicio: "2026-09-01".into(),
                fin: "2026-09-15".into()
            }
        );
        assert_eq!(
            periodo_de(fecha(2026, 12, 20)),
            Periodo {
                inicio: "2026-12-16".into(),
                fin: "2026-12-31".into()
            }
        );
    }

    #[test]
    fn debe_cerrar_el_tramo_16_fin_del_mes_anterior_el_dia_1() {
        assert_eq!(
            periodo_a_cerrar(fecha(2026, 1, 1)),
            Periodo {
                inicio: "2025-12-16".into(),
                fin: "2025-12-31".into()
            }
        );
    }

    #[test]
    fn debe_cerrar_la_primera_quincena_el_dia_15() {
        assert_eq!(
            periodo_a_cerrar(fecha(2026, 9, 15)),
            Periodo {
                inicio: "2026-09-01".into(),
                fin: "2026-09-15".into()
            }
        );
    }

    #[test]
    fn debe_cerrar_el_tramo_16_fin_del_mes_vigente_del_dia_16_en_adelante() {
        assert_eq!(
            periodo_a_cerrar(fecha(2026, 8, 20)),
            Periodo {
                inicio: "2026-08-16".into(),
                fin: "2026-08-31".into()
            }
        );
    }

    #[test]
    fn debe_manejar_febrero_y_anio_bisiesto() {
        assert_eq!(
            periodo_de(fecha(2028, 2, 20)),
            Periodo {
                inicio: "2028-02-16".into(),
                fin: "2028-02-29".into()
            }
        );
        assert_eq!(
            periodo_a_cerrar(fecha(2028, 3, 1)),
            Periodo {
                inicio: "2028-02-16".into(),
                fin: "2028-02-29".into()
            }
        );
    }

    #[test]
    fn debe_sumar_sin_perder_el_orden_de_aparicion() {
        let mut mapa = Vec::new();
        sumar(&mut mapa, "b".into(), 10);
        sumar(&mut mapa, "a".into(), 20);
        sumar(&mut mapa, "b".into(), 5);
        assert_eq!(mapa, vec![("b".to_string(), 15), ("a".to_string(), 20)]);
    }
}
