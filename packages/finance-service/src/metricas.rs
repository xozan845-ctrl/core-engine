//! Proyección CQRS de métricas mensuales (doc 8.8): materializa los eventos del
//! bus en agregados por mes para el tablero financiero.

use crate::errors::{error_db, ApiError};
use sqlx::{Postgres, Transaction};

pub const TIPOS_METRICA: [&str; 9] = [
    "gmv",
    "pedidos",
    "entregados",
    "devoluciones",
    "ingresos_comisiones",
    "carritos_iniciados",
    "checkouts_completados",
    "cortes_esperados",
    "liquidaciones_a_tiempo",
];

#[derive(Debug, Clone, Copy, Default)]
pub struct DeltasVendedor {
    pub ventas: i64,
    pub monto_cents: i64,
    pub comision_cents: i64,
    pub devoluciones: i64,
}

pub struct Metricas;

impl Metricas {
    pub async fn sumar_metrica(
        tx: &mut Transaction<'_, Postgres>,
        tipo: &str,
        monto_cents: i64,
        cantidad: i64,
    ) -> Result<(), ApiError> {
        if !TIPOS_METRICA.contains(&tipo) {
            return Err(ApiError::interno());
        }
        sqlx::query(
            "INSERT INTO finance.metricas_mensuales
               (periodo, tipo, monto_cents, cantidad, actualizado_en)
             VALUES (date_trunc('month', NOW())::date, $1, $2, $3, NOW())
             ON CONFLICT (periodo, tipo) DO UPDATE SET
               monto_cents  = finance.metricas_mensuales.monto_cents + EXCLUDED.monto_cents,
               cantidad     = finance.metricas_mensuales.cantidad + EXCLUDED.cantidad,
               actualizado_en = NOW()",
        )
        .bind(tipo)
        .bind(monto_cents as i32)
        .bind(cantidad as i32)
        .execute(&mut **tx)
        .await
        .map_err(error_db)?;
        Ok(())
    }

    pub async fn incrementar_vendedor(
        tx: &mut Transaction<'_, Postgres>,
        vendedor_id: &str,
        deltas: DeltasVendedor,
    ) -> Result<(), ApiError> {
        let vendedor = uuid::Uuid::parse_str(vendedor_id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El vendedor debe ser un UUID."))?;
        sqlx::query(
            "INSERT INTO finance.metricas_vendedores
               (vendedor_id, periodo, ventas, monto_cents, comision_cents, devoluciones,
                primera_venta_en, ultima_venta_en, actualizado_en)
             VALUES ($1, date_trunc('month', NOW())::date, $2, $3, $4, $5,
                     CASE WHEN $2 > 0 THEN NOW() ELSE NULL END, NOW(), NOW())
             ON CONFLICT (vendedor_id, periodo) DO UPDATE SET
               ventas          = finance.metricas_vendedores.ventas + EXCLUDED.ventas,
               monto_cents     = finance.metricas_vendedores.monto_cents + EXCLUDED.monto_cents,
               comision_cents  = finance.metricas_vendedores.comision_cents + EXCLUDED.comision_cents,
               devoluciones    = finance.metricas_vendedores.devoluciones + EXCLUDED.devoluciones,
               primera_venta_en = COALESCE(finance.metricas_vendedores.primera_venta_en,
                                           EXCLUDED.primera_venta_en),
               ultima_venta_en = EXCLUDED.ultima_venta_en,
               actualizado_en  = NOW()",
        )
        .bind(vendedor)
        .bind(deltas.ventas as i32)
        .bind(deltas.monto_cents as i32)
        .bind(deltas.comision_cents as i32)
        .bind(deltas.devoluciones as i32)
        .execute(&mut **tx)
        .await
        .map_err(error_db)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_listar_los_tipos_de_metrica_del_contrato() {
        for tipo in TIPOS_METRICA {
            assert!(!tipo.is_empty());
        }
        assert_eq!(TIPOS_METRICA.len(), 9);
    }
}
