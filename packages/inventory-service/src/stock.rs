//! Stock sobre `inventory.item_stock_ubicacion` (fuente única desde ADR-0020 fase 3b).

use crate::db::interno;
use crate::errors::ApiError;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

pub async fn total_en_transaccion(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
    item_id: &str,
) -> Result<i64, ApiError> {
    let total: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(stock), 0)::bigint
           FROM inventory.item_stock_ubicacion
          WHERE empresa_id = $1::uuid AND item_id = $2::uuid",
    )
    .bind(empresa_id)
    .bind(item_id)
    .fetch_one(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(total)
}

pub async fn aplicar_delta_en_transaccion(
    tx: &mut Transaction<'_, Postgres>,
    empresa_id: &str,
    item_id: &str,
    delta: i64,
) -> Result<i64, ApiError> {
    if delta == 0 {
        return total_en_transaccion(tx, empresa_id, item_id).await;
    }

    let default: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM inventory.ubicaciones
          WHERE empresa_id = $1::uuid AND activo AND predeterminada
          ORDER BY creado_en, id LIMIT 1",
    )
    .bind(empresa_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(interno)?;

    if delta > 0 {
        let ubicacion = match default {
            Some(id) => id,
            None => sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM inventory.ubicaciones
                  WHERE empresa_id = $1::uuid AND activo
                  ORDER BY creado_en, id LIMIT 1",
            )
            .bind(empresa_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(interno)?
            .ok_or_else(|| {
                ApiError::dominio(
                    "SIN_UBICACION",
                    "El item no tiene una ubicación activa para registrar stock.",
                )
            })?,
        };
        sqlx::query(
            "INSERT INTO inventory.item_stock_ubicacion AS s
               (empresa_id, item_id, ubicacion_id, stock)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4)
             ON CONFLICT (item_id, ubicacion_id) DO UPDATE
               SET stock = s.stock + EXCLUDED.stock, actualizado_en = NOW()",
        )
        .bind(empresa_id)
        .bind(item_id)
        .bind(ubicacion.to_string())
        .bind(delta as i32)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
    } else {
        let mut restante = -delta;
        let filas = sqlx::query(
            "SELECT s.ubicacion_id, s.stock
               FROM inventory.item_stock_ubicacion s
              WHERE s.empresa_id = $1::uuid AND s.item_id = $2::uuid AND s.stock > 0
              ORDER BY (s.ubicacion_id = $3::uuid) DESC, s.stock DESC, s.ubicacion_id
              FOR UPDATE",
        )
        .bind(empresa_id)
        .bind(item_id)
        .bind(default.map(|id| id.to_string()))
        .fetch_all(&mut **tx)
        .await
        .map_err(interno)?;
        for fila in filas {
            if restante <= 0 {
                break;
            }
            let ubicacion: Uuid = fila.try_get("ubicacion_id").map_err(interno)?;
            let ubicacion = ubicacion.to_string();
            let stock: i32 = fila.try_get("stock").map_err(interno)?;
            let quitar = restante.min(stock as i64) as i32;
            sqlx::query(
                "UPDATE inventory.item_stock_ubicacion
                    SET stock = stock - $1, actualizado_en = NOW()
                  WHERE item_id = $2::uuid AND ubicacion_id = $3::uuid",
            )
            .bind(quitar)
            .bind(item_id)
            .bind(ubicacion)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
            restante -= quitar as i64;
        }
        if restante > 0 {
            return Err(ApiError::dominio(
                "STOCK_INSUFICIENTE",
                "El stock físico del item no alcanza.",
            ));
        }
    }

    total_en_transaccion(tx, empresa_id, item_id).await
}
