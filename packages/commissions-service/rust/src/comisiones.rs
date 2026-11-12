//! Dominio y repositorio de comisiones (RN-04 y RN-06).
//!
//! Réplica de `comisiones.service.ts`: cálculo inmutable, idempotencia por
//! `(order_id, linea_clave)`, compensación de devoluciones y pago simulado.

use crate::db::{centavos_i32, error_db, parsear_uuid, timestamp, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use crate::money;
use async_trait::async_trait;
use serde::Serialize;
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Calculo {
    pub comision_cents: i64,
    pub monto_vendedor_cents: i64,
    pub monto_empresario_cents: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Comision {
    pub id: String,
    pub order_id: String,
    pub linea_clave: Option<String>,
    pub empresa_id: Option<String>,
    pub tienda_id: Option<String>,
    pub vendedor_id: String,
    pub venta_cents: i64,
    pub comision_cents: i64,
    pub monto_vendedor_cents: i64,
    pub monto_empresario_cents: Option<i64>,
    pub participacion_porcentaje: Option<i64>,
    pub estado: String,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Pago {
    pub id: String,
    pub order_id: String,
    pub monto_cents: i64,
    pub estado: String,
    pub metodo: String,
    pub creado_en: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatosLinea {
    pub order_id: String,
    pub linea_clave: String,
    pub empresa_id: String,
    pub tienda_id: String,
    pub vendedor_id: String,
    pub venta_cents: i64,
    pub participacion: i64,
}

impl Comision {
    pub(crate) fn desde_fila(fila: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: uuid_texto(fila, "id")?,
            order_id: uuid_texto(fila, "order_id")?,
            linea_clave: fila.try_get::<Option<String>, _>("linea_clave")?,
            empresa_id: uuid_opcional(fila, "empresa_id")?,
            tienda_id: uuid_opcional(fila, "tienda_id")?,
            vendedor_id: uuid_texto(fila, "vendedor_id")?,
            venta_cents: i64::from(fila.try_get::<i32, _>("venta_cents")?),
            comision_cents: i64::from(fila.try_get::<i32, _>("comision_cents")?),
            monto_vendedor_cents: i64::from(fila.try_get::<i32, _>("monto_vendedor_cents")?),
            monto_empresario_cents: fila
                .try_get::<Option<i32>, _>("monto_empresario_cents")?
                .map(i64::from),
            participacion_porcentaje: fila
                .try_get::<Option<i32>, _>("participacion_porcentaje")?
                .map(i64::from),
            estado: fila.try_get::<String, _>("estado")?,
            creado_en: timestamp(fila, "creado_en")?,
        })
    }
}

impl Pago {
    pub(crate) fn desde_fila(fila: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: uuid_texto(fila, "id")?,
            order_id: uuid_texto(fila, "order_id")?,
            monto_cents: i64::from(fila.try_get::<i32, _>("monto_cents")?),
            estado: fila.try_get::<String, _>("estado")?,
            metodo: fila.try_get::<String, _>("metodo")?,
            creado_en: timestamp(fila, "creado_en")?,
        })
    }
}

/// Cálculo puro de la comisión y el reparto (RN-04).
pub fn calcular_con_tasa(
    venta_cents: i64,
    participacion: i64,
    tasa: f64,
) -> Result<Calculo, ApiError> {
    if !(0..=100).contains(&participacion) {
        return Err(ApiError::dominio(
            "PARTICIPACION_INVALIDA",
            "La participacion debe estar entre 0 y 100 %.",
        ));
    }
    let venta = money::validar_centavos(venta_cents)?;
    let comision = money::comision(venta, tasa)?;
    let neto = money::restar(venta, comision)?;
    let vendedor = if participacion == 100 {
        neto
    } else if participacion == 0 {
        0
    } else {
        money::comision(neto, participacion as f64 / 100.0)?
    };
    Ok(Calculo {
        comision_cents: comision,
        monto_vendedor_cents: vendedor,
        monto_empresario_cents: money::restar(neto, vendedor)?,
    })
}

#[async_trait]
pub trait ComisionesPort: Send + Sync {
    fn calcular(&self, venta_cents: i64, participacion: i64) -> Result<Calculo, ApiError>;
    fn tasa(&self) -> f64;
    async fn comisiones_de(&self, vendedor_id: &str) -> Result<Vec<Comision>, ApiError>;
    async fn comisiones_por_orden(&self, order_id: &str) -> Result<Vec<Comision>, ApiError>;
}

pub struct Comisiones {
    db: Arc<Db>,
    tasa: f64,
}

impl Comisiones {
    pub fn nueva(db: Arc<Db>, tasa: f64) -> Self {
        Self { db, tasa }
    }

    pub fn calcular(&self, venta_cents: i64, participacion: i64) -> Result<Calculo, ApiError> {
        calcular_con_tasa(venta_cents, participacion, self.tasa)
    }

    /// TC-08: registra la comisión de una línea entregada. `None` si ya existía.
    pub async fn acreditar_linea(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        datos: &DatosLinea,
    ) -> Result<Option<Comision>, ApiError> {
        let calculo = self.calcular(datos.venta_cents, datos.participacion)?;
        let order_id = parsear_uuid(&datos.order_id)?;
        let existente = sqlx::query(
            "SELECT * FROM commissions.comisiones WHERE order_id = $1 AND linea_clave = $2",
        )
        .bind(order_id)
        .bind(&datos.linea_clave)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?;
        if existente.is_some() {
            return Ok(None); // idempotencia por orden y línea
        }

        let fila = sqlx::query(
            "INSERT INTO commissions.comisiones
                (id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents,
                 comision_cents, monto_vendedor_cents, monto_empresario_cents,
                 participacion_porcentaje, estado)
             VALUES (gen_random_uuid(), $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'acreditada')
             RETURNING *",
        )
        .bind(order_id)
        .bind(&datos.linea_clave)
        .bind(parsear_uuid(&datos.empresa_id)?)
        .bind(parsear_uuid(&datos.tienda_id)?)
        .bind(parsear_uuid(&datos.vendedor_id)?)
        .bind(centavos_i32(datos.venta_cents)?)
        .bind(centavos_i32(calculo.comision_cents)?)
        .bind(centavos_i32(calculo.monto_vendedor_cents)?)
        .bind(centavos_i32(calculo.monto_empresario_cents)?)
        .bind(centavos_i32(datos.participacion)?)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::dominio(
                "COMISION_FALLIDA",
                "No se pudo acreditar la comision.",
            ));
        };
        Comision::desde_fila(&fila).map(Some).map_err(error_db)
    }

    /// RN-06: la devolución compensa la comisión en la siguiente liquidación.
    pub async fn compensar(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        motivo: &str,
    ) -> Result<(), ApiError> {
        let order_uuid = parsear_uuid(order_id)?;
        let filas = sqlx::query("SELECT * FROM commissions.comisiones WHERE order_id = $1")
            .bind(order_uuid)
            .fetch_all(&mut **tx)
            .await
            .map_err(error_db)?;
        for fila in filas {
            let comision = Comision::desde_fila(&fila).map_err(error_db)?;
            if comision.estado == "compensada" {
                continue;
            }
            let monto_empresario = comision.monto_empresario_cents.unwrap_or(0);
            sqlx::query(
                "INSERT INTO commissions.compensaciones_devoluciones
                    (id, comision_id, order_id, vendedor_id, empresa_id, monto_cents,
                     monto_empresario_cents, motivo, creado_en)
                 VALUES (gen_random_uuid(), $1,$2,$3,$4,$5,$6,$7,NOW())",
            )
            .bind(parsear_uuid(&comision.id)?)
            .bind(order_uuid)
            .bind(parsear_uuid(&comision.vendedor_id)?)
            .bind(
                comision
                    .empresa_id
                    .as_deref()
                    .map(parsear_uuid)
                    .transpose()?,
            )
            .bind(centavos_i32(-comision.monto_vendedor_cents)?)
            .bind(centavos_i32(-monto_empresario)?)
            .bind(motivo)
            .execute(&mut **tx)
            .await
            .map_err(error_db)?;
            sqlx::query("UPDATE commissions.comisiones SET estado = 'compensada' WHERE id = $1")
                .bind(parsear_uuid(&comision.id)?)
                .execute(&mut **tx)
                .await
                .map_err(error_db)?;
        }
        Ok(())
    }

    /// Pago simulado del MVP (`payment.procesado`).
    pub async fn registrar_pago(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        monto_cents: i64,
    ) -> Result<Pago, ApiError> {
        let order_uuid = parsear_uuid(order_id)?;
        let existente = sqlx::query(
            "SELECT id, order_id, monto_cents, estado, metodo, creado_en
             FROM commissions.pagos WHERE order_id = $1",
        )
        .bind(order_uuid)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?;
        if let Some(fila) = existente {
            return Pago::desde_fila(&fila).map_err(error_db);
        }
        let fila = sqlx::query(
            "INSERT INTO commissions.pagos (id, order_id, monto_cents, estado, metodo)
             VALUES (gen_random_uuid(), $1, $2, 'procesado', 'simulado')
             RETURNING id, order_id, monto_cents, estado, metodo, creado_en",
        )
        .bind(order_uuid)
        .bind(centavos_i32(monto_cents)?)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Err(ApiError::dominio(
                "PAGO_FALLIDO",
                "No se pudo registrar el pago.",
            ));
        };
        Pago::desde_fila(&fila).map_err(error_db)
    }

    async fn listar(&self, sql: &str, valor: &str) -> Result<Vec<Comision>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(sql)
            .bind(parsear_uuid(valor)?)
            .fetch_all(&mut *tx)
            .await
            .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas
            .iter()
            .map(Comision::desde_fila)
            .collect::<Result<Vec<_>, _>>()
            .map_err(error_db)
    }
}

#[async_trait]
impl ComisionesPort for Comisiones {
    fn calcular(&self, venta_cents: i64, participacion: i64) -> Result<Calculo, ApiError> {
        Comisiones::calcular(self, venta_cents, participacion)
    }

    fn tasa(&self) -> f64 {
        self.tasa
    }

    async fn comisiones_de(&self, vendedor_id: &str) -> Result<Vec<Comision>, ApiError> {
        self.listar(
            "SELECT id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents,
                    comision_cents, monto_vendedor_cents, monto_empresario_cents,
                    participacion_porcentaje, estado, creado_en
             FROM commissions.comisiones WHERE vendedor_id = $1
             ORDER BY creado_en DESC LIMIT 200",
            vendedor_id,
        )
        .await
    }

    async fn comisiones_por_orden(&self, order_id: &str) -> Result<Vec<Comision>, ApiError> {
        self.listar(
            "SELECT id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents,
                    comision_cents, monto_vendedor_cents, monto_empresario_cents,
                    participacion_porcentaje, estado, creado_en
             FROM commissions.comisiones WHERE order_id = $1",
            order_id,
        )
        .await
    }
}

/// Implementación para tests: el repositorio no está disponible.
pub struct ComisionesNoDisponible;

#[async_trait]
impl ComisionesPort for ComisionesNoDisponible {
    fn calcular(&self, venta_cents: i64, participacion: i64) -> Result<Calculo, ApiError> {
        calcular_con_tasa(venta_cents, participacion, 0.12)
    }

    fn tasa(&self) -> f64 {
        0.12
    }

    async fn comisiones_de(&self, _vendedor_id: &str) -> Result<Vec<Comision>, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }

    async fn comisiones_por_orden(&self, _order_id: &str) -> Result<Vec<Comision>, ApiError> {
        Err(ApiError::servicio_no_disponible("commissions-service"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_cobrar_plataforma_primero_y_repartir_el_neto() {
        assert_eq!(
            calcular_con_tasa(10_000, 60, 0.12).expect("calculo"),
            Calculo {
                comision_cents: 1_200,
                monto_vendedor_cents: 5_280,
                monto_empresario_cents: 3_520,
            }
        );
    }

    #[test]
    fn debe_asignar_todo_el_neto_al_vendedor_con_100() {
        assert_eq!(
            calcular_con_tasa(10_000, 100, 0.12).expect("calculo"),
            Calculo {
                comision_cents: 1_200,
                monto_vendedor_cents: 8_800,
                monto_empresario_cents: 0,
            }
        );
    }

    #[test]
    fn debe_asignar_todo_el_neto_al_empresario_con_0() {
        assert_eq!(
            calcular_con_tasa(10_000, 0, 0.12).expect("calculo"),
            Calculo {
                comision_cents: 1_200,
                monto_vendedor_cents: 0,
                monto_empresario_cents: 8_800,
            }
        );
    }

    #[test]
    fn debe_rechazar_participaciones_fuera_de_rango() {
        for participacion in [-1_i64, 101, 1_000] {
            let error = calcular_con_tasa(10_000, participacion, 0.12).expect_err("participacion");
            assert_eq!(error.codigo, "PARTICIPACION_INVALIDA");
        }
    }

    #[test]
    fn debe_ser_estable_para_ventas_de_un_centavo() {
        for participacion in 0..=100 {
            let calculo = calcular_con_tasa(1, participacion, 0.12).expect("calculo");
            assert_eq!(
                1 - calculo.comision_cents,
                calculo.monto_vendedor_cents + calculo.monto_empresario_cents
            );
        }
    }

    #[test]
    fn debe_conservar_el_reparto_en_todo_el_rango_de_participacion() {
        for participacion in 0..=100 {
            let calculo = calcular_con_tasa(115_000, participacion, 0.12).expect("calculo");
            let neto = 115_000 - calculo.comision_cents;
            assert_eq!(
                calculo.monto_vendedor_cents + calculo.monto_empresario_cents,
                neto
            );
            assert!(calculo.monto_vendedor_cents >= 0);
            assert!(calculo.monto_empresario_cents >= 0);
        }
    }
}
