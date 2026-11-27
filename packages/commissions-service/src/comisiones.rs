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
    pub negocio_id: Option<String>,
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
    pub proveedor: String,
    pub moneda: String,
    pub comision_pasarela_cents: i64,
    pub impuesto_pasarela_cents: i64,
    pub retencion_cents: i64,
    pub monto_liquidado_cents: i64,
    pub moneda_liquidacion: String,
    pub creado_en: String,
}

/// Datos de pago que llegan en `payment.procesado`. Los campos de liquidación
/// son opcionales para no romper eventos antiguos.
#[derive(Debug, Clone, Default)]
pub struct DatosPago {
    pub order_id: String,
    pub monto_cents: i64,
    pub metodo: String,
    pub proveedor: String,
    pub moneda: String,
    pub comision_pasarela_cents: i64,
    pub impuesto_pasarela_cents: i64,
    pub retencion_cents: i64,
    pub monto_liquidado_cents: i64,
    pub moneda_liquidacion: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatosLinea {
    pub order_id: String,
    pub linea_clave: String,
    pub empresa_id: String,
    pub negocio_id: Option<String>,
    pub tienda_id: String,
    pub vendedor_id: String,
    pub venta_cents: i64,
    pub participacion: i64,
    pub cantidad: i64,
}

impl Comision {
    pub(crate) fn desde_fila(fila: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: uuid_texto(fila, "id")?,
            order_id: uuid_texto(fila, "order_id")?,
            linea_clave: fila.try_get::<Option<String>, _>("linea_clave")?,
            empresa_id: uuid_opcional(fila, "empresa_id")?,
            negocio_id: uuid_opcional(fila, "negocio_id")?,
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
            estado: fila.try_get("estado")?,
            metodo: fila.try_get("metodo")?,
            proveedor: fila.try_get("proveedor")?,
            moneda: fila.try_get("moneda")?,
            comision_pasarela_cents: i64::from(fila.try_get::<i32, _>("comision_pasarela_cents")?),
            impuesto_pasarela_cents: i64::from(fila.try_get::<i32, _>("impuesto_pasarela_cents")?),
            retencion_cents: i64::from(fila.try_get::<i32, _>("retencion_cents")?),
            monto_liquidado_cents: i64::from(fila.try_get::<i32, _>("monto_liquidado_cents")?),
            moneda_liquidacion: fila.try_get("moneda_liquidacion")?,
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

/// Redondeo proporcional entero (medio hacia arriba), acotado a `total_parte`.
fn proporcional(total: i64, parte: i64, total_parte: i64) -> i64 {
    if total <= 0 || parte <= 0 || total_parte <= 0 {
        return 0;
    }
    let parte = parte.min(total_parte);
    ((i128::from(total) * i128::from(parte) + i128::from(total_parte) / 2)
        / i128::from(total_parte)) as i64
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
                (id, order_id, linea_clave, empresa_id, negocio_id, tienda_id, vendedor_id, venta_cents,
                 comision_cents, monto_vendedor_cents, monto_empresario_cents,
                 participacion_porcentaje, cantidad, estado)
             VALUES (gen_random_uuid(), $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,'acreditada')
             RETURNING *",
        )
        .bind(order_id)
        .bind(&datos.linea_clave)
        .bind(parsear_uuid(&datos.empresa_id)?)
        .bind(
            datos
                .negocio_id
                .as_deref()
                .map(parsear_uuid)
                .transpose()?,
        )
        .bind(parsear_uuid(&datos.tienda_id)?)
        .bind(parsear_uuid(&datos.vendedor_id)?)
        .bind(centavos_i32(datos.venta_cents)?)
        .bind(centavos_i32(calculo.comision_cents)?)
        .bind(centavos_i32(calculo.monto_vendedor_cents)?)
        .bind(centavos_i32(calculo.monto_empresario_cents)?)
        .bind(centavos_i32(datos.participacion)?)
        .bind(datos.cantidad as i32)
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

    /// RN-06 parcial: compensa una cantidad devuelta de una línea. La
    /// compensación es acumulativa: varias devoluciones de la misma línea nunca
    /// superan la cantidad y los montos originalmente acreditados.
    pub async fn compensar_linea(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        linea_clave: &str,
        cantidad: i64,
        motivo: &str,
    ) -> Result<(), ApiError> {
        if cantidad <= 0 {
            return Ok(());
        }
        let order_uuid = parsear_uuid(order_id)?;
        let fila = sqlx::query(
            "SELECT * FROM commissions.comisiones
              WHERE order_id = $1 AND linea_clave = $2 FOR UPDATE",
        )
        .bind(order_uuid)
        .bind(linea_clave)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?;
        let Some(fila) = fila else {
            return Ok(());
        };
        let comision = Comision::desde_fila(&fila).map_err(error_db)?;
        if comision.estado == "compensada" {
            return Ok(());
        }
        let cantidad_original = i64::from(fila.try_get::<i32, _>("cantidad").unwrap_or(1).max(1));
        let fila_comision = parsear_uuid(&comision.id)?;

        let previa: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(cantidad), 0)::bigint,
                    COALESCE(-SUM(monto_cents), 0)::bigint,
                    COALESCE(-SUM(monto_empresario_cents), 0)::bigint,
                    COALESCE(-SUM(monto_comision_cents), 0)::bigint
               FROM commissions.compensaciones_devoluciones
              WHERE comision_id = $1",
        )
        .bind(fila_comision)
        .fetch_one(&mut **tx)
        .await
        .map_err(error_db)?;
        let nueva = (previa.0 + cantidad).min(cantidad_original);
        if nueva <= previa.0 {
            return Ok(());
        }
        let delta_vendedor =
            proporcional(comision.monto_vendedor_cents, nueva, cantidad_original) - previa.1;
        let delta_empresario = proporcional(
            comision.monto_empresario_cents.unwrap_or(0),
            nueva,
            cantidad_original,
        ) - previa.2;
        let delta_comision =
            proporcional(comision.comision_cents, nueva, cantidad_original) - previa.3;
        if delta_vendedor <= 0 && delta_empresario <= 0 && delta_comision <= 0 {
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO commissions.compensaciones_devoluciones
                (id, comision_id, order_id, vendedor_id, empresa_id, negocio_id, monto_cents,
                 monto_empresario_cents, monto_comision_cents, cantidad, motivo, creado_en)
             VALUES (gen_random_uuid(), $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NOW())",
        )
        .bind(fila_comision)
        .bind(order_uuid)
        .bind(parsear_uuid(&comision.vendedor_id)?)
        .bind(
            comision
                .empresa_id
                .as_deref()
                .map(parsear_uuid)
                .transpose()?,
        )
        .bind(
            comision
                .negocio_id
                .as_deref()
                .map(parsear_uuid)
                .transpose()?,
        )
        .bind(centavos_i32(-delta_vendedor)?)
        .bind(centavos_i32(-delta_empresario)?)
        .bind(centavos_i32(-delta_comision)?)
        .bind((nueva - previa.0) as i32)
        .bind(motivo)
        .execute(&mut **tx)
        .await
        .map_err(error_db)?;
        if nueva >= cantidad_original {
            sqlx::query("UPDATE commissions.comisiones SET estado = 'compensada' WHERE id = $1")
                .bind(fila_comision)
                .execute(&mut **tx)
                .await
                .map_err(error_db)?;
        }
        Ok(())
    }

    /// RN-06: la devolución total compensa lo que aún quede pendiente en cada
    /// línea. Las devoluciones parciales previas ya descontaron su parte.
    pub async fn compensar(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        motivo: &str,
    ) -> Result<(), ApiError> {
        let order_uuid = parsear_uuid(order_id)?;
        let filas = sqlx::query(
            "SELECT linea_clave, cantidad FROM commissions.comisiones WHERE order_id = $1",
        )
        .bind(order_uuid)
        .fetch_all(&mut **tx)
        .await
        .map_err(error_db)?;
        for fila in filas {
            let Some(linea_clave) = fila
                .try_get::<Option<String>, _>("linea_clave")
                .ok()
                .flatten()
            else {
                continue;
            };
            let cantidad = i64::from(fila.try_get::<i32, _>("cantidad").unwrap_or(1).max(1));
            self.compensar_linea(tx, order_id, &linea_clave, cantidad, motivo)
                .await?;
        }
        Ok(())
    }

    /// Pago confirmado por la pasarela (`payment.procesado`).
    pub async fn registrar_pago(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        datos: &DatosPago,
    ) -> Result<Pago, ApiError> {
        let order_uuid = parsear_uuid(&datos.order_id)?;
        let existente = sqlx::query(
            "SELECT id, order_id, monto_cents, estado, metodo, proveedor, moneda,
                    comision_pasarela_cents, impuesto_pasarela_cents, retencion_cents,
                    monto_liquidado_cents, moneda_liquidacion, creado_en
             FROM commissions.pagos WHERE order_id = $1",
        )
        .bind(order_uuid)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?;
        if let Some(fila) = existente {
            return Pago::desde_fila(&fila).map_err(error_db);
        }
        let metodo = if datos.metodo.trim().is_empty() {
            "simulado"
        } else {
            datos.metodo.as_str()
        };
        let fila = sqlx::query(
            "INSERT INTO commissions.pagos
               (id, order_id, monto_cents, estado, metodo, proveedor, moneda,
                comision_pasarela_cents, impuesto_pasarela_cents, retencion_cents,
                monto_liquidado_cents, moneda_liquidacion)
             VALUES (gen_random_uuid(), $1, $2, 'procesado', $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING id, order_id, monto_cents, estado, metodo, proveedor, moneda,
                       comision_pasarela_cents, impuesto_pasarela_cents, retencion_cents,
                       monto_liquidado_cents, moneda_liquidacion, creado_en",
        )
        .bind(order_uuid)
        .bind(centavos_i32(datos.monto_cents)?)
        .bind(metodo)
        .bind(&datos.proveedor)
        .bind(&datos.moneda)
        .bind(centavos_i32(datos.comision_pasarela_cents)?)
        .bind(centavos_i32(datos.impuesto_pasarela_cents)?)
        .bind(centavos_i32(datos.retencion_cents)?)
        .bind(centavos_i32(datos.monto_liquidado_cents)?)
        .bind(&datos.moneda_liquidacion)
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
            "SELECT id, order_id, linea_clave, empresa_id, negocio_id, tienda_id, vendedor_id, venta_cents,
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
            "SELECT id, order_id, linea_clave, empresa_id, negocio_id, tienda_id, vendedor_id, venta_cents,
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
