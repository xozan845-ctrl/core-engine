use super::helpers::*;
use super::*;

impl Productos {
    pub async fn reservar_stock(&self, order_id: &str, items: &Value) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let salida = self
            .reservar_stock_en_transaccion(&mut tx, order_id, items)
            .await?;
        tx.commit().await.map_err(interno)?;
        Ok(salida)
    }

    /// RN-03: descuento atómico de stock dentro de una transacción.
    ///
    /// Si alguna línea no alcanza stock se rechaza la reserva completa: se
    /// deshace el SAVEPOINT y la transacción del llamador sigue viva para que
    /// registre la idempotencia y emita `stock.fallido`.
    pub async fn reservar_stock_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let lineas = lineas_de(items);
        let lineas = self.expandir_componentes_lineas(tx, &lineas).await?;
        sqlx::query("SAVEPOINT reserva_stock")
            .execute(&mut **tx)
            .await
            .map_err(interno)?;

        let mut fallidos: Vec<Value> = Vec::new();
        for item in &lineas {
            // Bloqueo pesimista requerido por la regla dorada (Data Integrity).
            let lock = sqlx::query(
                "SELECT i.stock, i.id AS item_id, i.track_serial, i.garantia_meses,
                        EXISTS (SELECT 1 FROM catalog.items v
                                 WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL) AS tiene_variantes
                   FROM catalog.items i
                  WHERE i.empresa_id = $1::uuid AND LOWER(i.codigo) = LOWER($2)
                    AND i.tipo = 'producto' AND i.estado = 'activo'
                    AND i.eliminado_en IS NULL FOR UPDATE",
            )
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .fetch_optional(&mut **tx)
            .await
            .map_err(interno)?;
            let Some(lock) = lock else {
                fallidos.push(fallido(item, "stock_insuficiente"));
                continue;
            };
            let stock: Option<i32> = lock.get("stock");
            if (stock.unwrap_or(0) as i64) < item.cantidad {
                fallidos.push(fallido(item, "stock_insuficiente"));
                continue;
            }
            let item_id: Uuid = lock.get("item_id");
            let track_serial: bool = lock.get("track_serial");
            let garantia_meses: Option<i32> = lock.get("garantia_meses");
            let tiene_variantes: bool = lock.get("tiene_variantes");
            if tiene_variantes {
                fallidos.push(fallido(item, "variante_requerida"));
                continue;
            }

            let lotes = sqlx::query(
                "SELECT id, cantidad_disponible,
                        (vence_en IS NOT NULL AND vence_en < CURRENT_DATE) AS vencido
                   FROM catalog.lotes_inventario
                  WHERE empresa_id = $1::uuid AND item_id = $2::uuid AND cantidad_disponible > 0
                  ORDER BY vence_en NULLS LAST, creado_en, id
                  FOR UPDATE",
            )
            .bind(&item.empresa_id)
            .bind(item_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(interno)?;
            if !lotes.is_empty() {
                let disponibles = lotes
                    .iter()
                    .map(|lote| {
                        (
                            lote.get::<Uuid, _>("id"),
                            lote.get::<i32, _>("cantidad_disponible") as i64,
                            lote.get::<bool, _>("vencido"),
                        )
                    })
                    .collect::<Vec<_>>();
                match planificar_fefo(item.cantidad, &disponibles) {
                    Ok(asignaciones) => {
                        for (lote_id, cantidad) in asignaciones {
                            sqlx::query(
                                "UPDATE catalog.lotes_inventario
                                    SET cantidad_disponible = cantidad_disponible - $1,
                                        actualizado_en = NOW()
                                  WHERE id = $2::uuid",
                            )
                            .bind(cantidad)
                            .bind(lote_id)
                            .execute(&mut **tx)
                            .await
                            .map_err(interno)?;
                            sqlx::query(
                                "INSERT INTO catalog.reservas_lotes
                                   (order_id, lote_id, empresa_id, item_id, cantidad)
                                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5)",
                            )
                            .bind(order_id)
                            .bind(lote_id)
                            .bind(&item.empresa_id)
                            .bind(item_id)
                            .bind(cantidad)
                            .execute(&mut **tx)
                            .await
                            .map_err(interno)?;
                        }
                    }
                    Err(_) => {
                        fallidos.push(fallido(item, "lote_vencido"));
                        continue;
                    }
                }
            }

            if track_serial {
                let series = sqlx::query(
                    "SELECT s.id
                       FROM catalog.item_series s
                       LEFT JOIN catalog.ubicaciones u ON u.id = s.ubicacion_id
                      WHERE s.empresa_id = $1::uuid AND s.item_id = $2::uuid
                        AND s.estado = 'disponible'
                      ORDER BY (u.predeterminada IS TRUE) DESC, s.creado_en, s.id
                      LIMIT $3
                      FOR UPDATE OF s",
                )
                .bind(&item.empresa_id)
                .bind(item_id)
                .bind(item.cantidad)
                .fetch_all(&mut **tx)
                .await
                .map_err(interno)?;
                if !seriales_suficientes(series.len(), item.cantidad) {
                    fallidos.push(fallido(item, "serial_insuficiente"));
                    continue;
                }
                let ids = series
                    .iter()
                    .map(|fila| fila.get::<Uuid, _>("id"))
                    .collect::<Vec<_>>();
                sqlx::query(
                    "UPDATE catalog.item_series
                        SET estado = 'vendido', order_id = $1::uuid, vendido_en = NOW(),
                            ubicacion_id = NULL,
                            garantia_hasta = CASE WHEN $3::int IS NULL THEN NULL
                              ELSE (CURRENT_DATE + ($3::int || ' months')::interval)::date END,
                            actualizado_en = NOW()
                      WHERE id = ANY($2::uuid[])",
                )
                .bind(order_id)
                .bind(&ids)
                .bind(garantia_meses)
                .execute(&mut **tx)
                .await
                .map_err(interno)?;
            }

            sqlx::query(
                "UPDATE catalog.items
                    SET stock = stock - $3, actualizado_en = NOW()
                  WHERE empresa_id = $1::uuid AND LOWER(codigo) = LOWER($2)
                    AND tipo = 'producto' AND eliminado_en IS NULL
                  RETURNING codigo",
            )
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .bind(item.cantidad)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        }

        if !fallidos.is_empty() {
            sqlx::query("ROLLBACK TO SAVEPOINT reserva_stock")
                .execute(&mut **tx)
                .await
                .map_err(interno)?;
            sqlx::query("RELEASE SAVEPOINT reserva_stock")
                .execute(&mut **tx)
                .await
                .map_err(interno)?;
            return Ok(json!({ "ok": false, "fallidos": fallidos }));
        }

        sqlx::query(
            "INSERT INTO catalog.reservas_ordenes (order_id, items_json, creado_en)
             VALUES ($1::uuid, $2::jsonb, NOW())",
        )
        .bind(order_id)
        .bind(lineas_json(&lineas).to_string())
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        sqlx::query("RELEASE SAVEPOINT reserva_stock")
            .execute(&mut **tx)
            .await
            .map_err(interno)?;

        let claves = claves_stock_de_lineas(&lineas);
        let stock_restante = self
            .stock_actual_de_en_transaccion(tx, &Value::Array(claves))
            .await?;
        Ok(json!({
            "ok": true,
            "fallidos": fallidos,
            "stock_restante": stock_restante,
            "reservados": lineas_json(&lineas),
        }))
    }

    /// Explota la composición (BOM) de cada línea: si el item tiene
    /// componentes se reservan las hojas multiplicadas por la cantidad vendida.
    pub async fn expandir_componentes(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let lineas = lineas_de(items);
        let expandidas = self.expandir_componentes_lineas(tx, &lineas).await?;
        Ok(lineas_json(&expandidas))
    }

    pub(super) async fn expandir_componentes_lineas(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        items: &[Linea],
    ) -> Result<Vec<Linea>, ApiError> {
        let mut acumuladas: Vec<Linea> = Vec::new();
        let mut indices: HashMap<String, usize> = HashMap::new();
        for item in items {
            let item_id = item.item_id.as_deref().unwrap_or("");
            if item_id.is_empty() {
                acumular(&mut acumuladas, &mut indices, item.clone());
                continue;
            }
            let resultado = sqlx::query(
                "WITH RECURSIVE bom(item_id, cantidad, ruta) AS (
                   SELECT c.componente_item_id, c.cantidad, ARRAY[$1::uuid, c.componente_item_id]
                     FROM catalog.item_componentes c
                    WHERE c.empresa_id = $2::uuid AND c.item_id = $1::uuid
                   UNION ALL
                   SELECT c.componente_item_id, bom.cantidad * c.cantidad, bom.ruta || c.componente_item_id
                     FROM bom
                     JOIN catalog.item_componentes c
                       ON c.item_id = bom.item_id AND c.empresa_id = $2::uuid
                    WHERE NOT (c.componente_item_id = ANY(bom.ruta))
                      AND array_length(bom.ruta, 1) < 6
                 )
                 SELECT i.id AS item_id, i.codigo AS sku, SUM(bom.cantidad)::int AS cantidad
                   FROM bom
                   JOIN catalog.items i
                     ON i.id = bom.item_id AND i.eliminado_en IS NULL AND i.tipo = 'producto'
                  WHERE NOT EXISTS (
                    SELECT 1 FROM catalog.item_componentes c2
                     WHERE c2.item_id = i.id AND c2.empresa_id = $2::uuid
                  )
                  GROUP BY i.id, i.codigo",
            )
            .bind(item_id)
            .bind(&item.empresa_id)
            .fetch_all(&mut **tx)
            .await
            .map_err(interno)?;
            if resultado.is_empty() {
                acumular(&mut acumuladas, &mut indices, item.clone());
                continue;
            }
            for fila in resultado {
                let componente_id: Uuid = fila.get("item_id");
                let sku: String = fila.get("sku");
                let cantidad: i32 = fila.get("cantidad");
                acumular(
                    &mut acumuladas,
                    &mut indices,
                    Linea {
                        empresa_id: item.empresa_id.clone(),
                        sku,
                        cantidad: (cantidad as i64) * item.cantidad,
                        item_id: Some(componente_id.to_string()),
                    },
                );
            }
        }
        Ok(acumuladas)
    }

    /// RN-06: devolución aprobada revierte stock por `(empresa, SKU)`.
    pub async fn reintegrar_stock(&self, order_id: &str, items: &Value) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        self.reintegrar_stock_en_transaccion(&mut tx, order_id, items)
            .await?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    /// Marca como devueltas las unidades vendidas de una orden. Si hay
    /// devolución parcial toma primero las unidades más antiguas.
    pub async fn devolver_series_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<(), ApiError> {
        for item in lineas_de(items) {
            sqlx::query(
                "WITH objetivo AS (
                   SELECT s.id
                     FROM catalog.item_series s
                     JOIN catalog.items i ON i.id = s.item_id
                    WHERE s.order_id = $1::uuid AND s.estado = 'vendido'
                      AND i.empresa_id = $2::uuid AND LOWER(i.codigo) = LOWER($3)
                    ORDER BY s.vendido_en, s.id
                    LIMIT $4
                    FOR UPDATE OF s
                 )
                 UPDATE catalog.item_series s
                    SET estado = 'devuelto', devuelto_en = NOW(),
                        ubicacion_id = COALESCE((
                          SELECT u.id FROM catalog.ubicaciones u
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

    pub async fn reintegrar_stock_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        items: &Value,
    ) -> Result<(), ApiError> {
        let lineas = lineas_de(items);
        for item in &lineas {
            sqlx::query(
                "UPDATE catalog.items
                    SET stock = stock + $3, actualizado_en = NOW()
                  WHERE empresa_id = $1::uuid AND LOWER(codigo) = LOWER($2)
                    AND tipo = 'producto' AND eliminado_en IS NULL",
            )
            .bind(&item.empresa_id)
            .bind(&item.sku)
            .bind(item.cantidad)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        }
        sqlx::query(
            "INSERT INTO catalog.ajustes_stock
               (empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo, realizado_en)
             SELECT i.empresa_id, i.id, i.stock - x.cantidad, i.stock,
                    'devolucion orden ' || $1, NOW()
               FROM catalog.items i
               JOIN jsonb_to_recordset($2::jsonb) AS x(empresa_id uuid, sku text, cantidad int)
                 ON i.empresa_id = x.empresa_id AND LOWER(i.codigo) = LOWER(x.sku)
              WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL",
        )
        .bind(order_id)
        .bind(lineas_json(&lineas).to_string())
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        // Restaura exactamente las asignaciones FEFO de la reserva.
        sqlx::query(
            "UPDATE catalog.lotes_inventario l
                SET cantidad_disponible = LEAST(l.cantidad_inicial, l.cantidad_disponible + r.cantidad),
                    actualizado_en = NOW()
               FROM catalog.reservas_lotes r
              WHERE r.lote_id = l.id AND r.order_id = $1::uuid",
        )
        .bind(order_id)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        sqlx::query("DELETE FROM catalog.reservas_lotes WHERE order_id = $1::uuid")
            .bind(order_id)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        // La reserva de esta orden ya no está activa tras reintegrar el stock.
        sqlx::query("DELETE FROM catalog.reservas_ordenes WHERE order_id = $1::uuid")
            .bind(order_id)
            .execute(&mut **tx)
            .await
            .map_err(interno)?;
        Ok(())
    }
}
