use super::*;

impl Reservas {
    /// Libera la reserva completa de una orden. Devuelve `false` si la orden no
    /// tiene reserva vigente (ya se liberó, falló la reserva o el evento llegó
    /// antes que `order.created`), lo que hace la operación idempotente entre
    /// distintos eventos de cancelación/devolución.
    pub async fn reintegrar_en_transaccion(
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<bool, ApiError> {
        let reserva: Option<Uuid> = sqlx::query_scalar(
            "SELECT order_id FROM inventory.reservas_ordenes
              WHERE order_id = $1::uuid FOR UPDATE",
        )
        .bind(order_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?;
        if reserva.is_none() {
            return Ok(false);
        }
        let lineas = lineas_de(items);
        for item in &lineas {
            let item_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM item.items
                  WHERE empresa_id = $1::uuid AND LOWER(codigo) = LOWER($2)
                    AND tipo = 'producto' AND eliminado_en IS NULL",
            )
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .fetch_optional(&mut **tx)
            .await
            .map_err(interno)?;
            let Some(item_id) = item_id else { continue };
            let ubicacion = item
                .ubicacion_id
                .as_deref()
                .and_then(|valor| Uuid::parse_str(valor).ok());
            let (anterior, nuevo) = if let Some(ubicacion) = ubicacion {
                let anterior = crate::stock::stock_de_ubicacion_en_transaccion(
                    tx,
                    &item.empresa_id,
                    &item_id.to_string(),
                    ubicacion,
                )
                .await?;
                let nuevo = crate::stock::aplicar_delta_en_ubicacion_en_transaccion(
                    tx,
                    &item.empresa_id,
                    &item_id.to_string(),
                    ubicacion,
                    item.cantidad,
                )
                .await?;
                (anterior, nuevo)
            } else {
                let anterior =
                    crate::stock::total_en_transaccion(tx, &item.empresa_id, &item_id.to_string())
                        .await?;
                let nuevo = crate::stock::aplicar_delta_en_transaccion(
                    tx,
                    &item.empresa_id,
                    &item_id.to_string(),
                    item.cantidad,
                )
                .await?;
                (anterior, nuevo)
            };
            sqlx::query(
                "INSERT INTO inventory.ajustes_stock
                   (empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, NOW())",
            )
            .bind(&item.empresa_id)
            .bind(item_id)
            .bind(anterior as i32)
            .bind(nuevo as i32)
            .bind(format!("devolucion orden {order_id}"))
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        }
        sqlx::query(
            "UPDATE inventory.lotes_inventario l
                SET cantidad_disponible = LEAST(l.cantidad_inicial, l.cantidad_disponible + r.cantidad),
                    actualizado_en = NOW()
               FROM inventory.reservas_lotes r
              WHERE r.lote_id = l.id AND r.order_id = $1::uuid",
        )
        .bind(order_id)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        sqlx::query("DELETE FROM inventory.reservas_lotes WHERE order_id = $1::uuid")
            .bind(order_id)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        sqlx::query("DELETE FROM inventory.reservas_ordenes WHERE order_id = $1::uuid")
            .bind(order_id)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        Ok(true)
    }

    /// Restock parcial por RMA. Idempotente por `devolucion_id`.
    pub async fn reintegrar_lineas_en_transaccion(
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        devolucion_id: &str,
        items: &Value,
    ) -> Result<(), ApiError> {
        let procesada: Option<Uuid> = sqlx::query_scalar(
            "SELECT devolucion_id FROM inventory.devoluciones_procesadas
              WHERE devolucion_id = $1::uuid",
        )
        .bind(devolucion_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?;
        if procesada.is_some() {
            return Ok(());
        }
        let lineas = lineas_de(items);
        for item in &lineas {
            let item_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM item.items
                  WHERE empresa_id = $1::uuid AND LOWER(codigo) = LOWER($2)
                    AND tipo = 'producto' AND eliminado_en IS NULL",
            )
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .fetch_optional(&mut **tx)
            .await
            .map_err(interno)?;
            let Some(item_id) = item_id else { continue };
            let ubicacion = item
                .ubicacion_id
                .as_deref()
                .and_then(|valor| Uuid::parse_str(valor).ok());
            let anterior = if let Some(ubicacion) = ubicacion {
                crate::stock::stock_de_ubicacion_en_transaccion(
                    tx,
                    &item.empresa_id,
                    &item_id.to_string(),
                    ubicacion,
                )
                .await?
            } else {
                crate::stock::total_en_transaccion(tx, &item.empresa_id, &item_id.to_string())
                    .await?
            };
            let nuevo = if let Some(ubicacion) = ubicacion {
                crate::stock::aplicar_delta_en_ubicacion_en_transaccion(
                    tx,
                    &item.empresa_id,
                    &item_id.to_string(),
                    ubicacion,
                    item.cantidad,
                )
                .await?
            } else {
                crate::stock::aplicar_delta_en_transaccion(
                    tx,
                    &item.empresa_id,
                    &item_id.to_string(),
                    item.cantidad,
                )
                .await?
            };
            sqlx::query(
                "INSERT INTO inventory.ajustes_stock
                   (empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, NOW())",
            )
            .bind(&item.empresa_id)
            .bind(item_id)
            .bind(anterior as i32)
            .bind(nuevo as i32)
            .bind(format!("devolucion rma {devolucion_id}"))
            .execute(&mut **tx)
            .await
            .map_err(interno)?;

            sqlx::query(
                "WITH objetivo AS (
                   SELECT s.id FROM item.item_series s
                    WHERE s.order_id = $1::uuid AND s.estado = 'vendido'
                      AND s.item_id = $2::uuid
                    ORDER BY s.vendido_en, s.id LIMIT $3
                    FOR UPDATE OF s
                 )
                 UPDATE item.item_series s
                    SET estado = 'disponible', order_id = NULL, vendido_en = NULL,
                        ubicacion_id = COALESCE($4::uuid, s.ubicacion_id),
                        actualizado_en = NOW()
                   FROM objetivo o WHERE s.id = o.id",
            )
            .bind(order_id)
            .bind(item_id)
            .bind(item.cantidad)
            .bind(ubicacion)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;

            let lotes = sqlx::query(
                "SELECT lote_id, cantidad FROM inventory.reservas_lotes
                  WHERE order_id = $1::uuid AND item_id = $2::uuid AND cantidad > 0
                  ORDER BY lote_id FOR UPDATE",
            )
            .bind(order_id)
            .bind(item_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(interno)?;
            let mut restante = item.cantidad;
            for lote in lotes {
                if restante <= 0 {
                    break;
                }
                let lote_id: Uuid = lote.try_get("lote_id").map_err(interno)?;
                let reservado: i32 = lote.try_get("cantidad").map_err(interno)?;
                let devolver = (reservado as i64).min(restante) as i32;
                sqlx::query(
                    "UPDATE inventory.lotes_inventario
                        SET cantidad_disponible = LEAST(cantidad_inicial, cantidad_disponible + $1),
                            actualizado_en = NOW()
                      WHERE id = $2::uuid",
                )
                .bind(devolver)
                .bind(lote_id)
                .execute(&mut **tx)
                .await
                .map_err(interno)?;
                sqlx::query(
                    "UPDATE inventory.reservas_lotes SET cantidad = cantidad - $1
                      WHERE order_id = $2::uuid AND lote_id = $3::uuid",
                )
                .bind(devolver)
                .bind(order_id)
                .bind(lote_id)
                .execute(&mut **tx)
                .await
                .map_err(interno)?;
                restante -= devolver as i64;
            }
        }
        sqlx::query(
            "INSERT INTO inventory.devoluciones_procesadas (devolucion_id, order_id)
             VALUES ($1::uuid, $2::uuid) ON CONFLICT (devolucion_id) DO NOTHING",
        )
        .bind(devolucion_id)
        .bind(order_id)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        Ok(())
    }

    pub async fn devolver_series_en_transaccion(
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<(), ApiError> {
        for item in lineas_de(items) {
            sqlx::query(
                "WITH objetivo AS (
                   SELECT s.id
                     FROM item.item_series s
                     JOIN item.items i ON i.id = s.item_id
                    WHERE s.order_id = $1::uuid AND s.estado = 'vendido'
                      AND i.empresa_id = $2::uuid AND LOWER(i.codigo) = LOWER($3)
                    ORDER BY s.vendido_en, s.id
                    LIMIT $4
                    FOR UPDATE OF s
                 )
                 UPDATE item.item_series s
                    SET estado = 'devuelto', devuelto_en = NOW(),
                        ubicacion_id = COALESCE((
                          SELECT u.id FROM inventory.ubicaciones u
                           WHERE u.empresa_id = $2::uuid AND u.predeterminada AND u.activo
                           LIMIT 1
                        ), s.ubicacion_id),
                        actualizado_en = NOW()
                   FROM objetivo o
                  WHERE s.id = o.id",
            )
            .bind(order_id)
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .bind(item.cantidad)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        }
        Ok(())
    }
}
