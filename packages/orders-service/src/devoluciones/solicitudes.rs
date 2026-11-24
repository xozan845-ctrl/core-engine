use super::prelude::*;
use super::*;

impl Devoluciones {
    pub async fn crear(
        &self,
        order_id: &str,
        actor: &UsuarioContexto,
        input: CrearDevolucionInput,
    ) -> Result<Devolucion, ApiError> {
        let orden = self
            .vistas
            .encontrar(&self.db, order_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))?;
        autorizar(&orden, actor)?;
        if input.items.is_empty() {
            return Err(ApiError::dominio(
                "DEVOLUCION_VACIA",
                "La devolución debe incluir al menos una línea.",
            ));
        }
        let ya_devuelto = self.cantidades_devueltas(order_id).await?;
        let mut lineas = Vec::with_capacity(input.items.len());
        let mut total = 0i64;
        let mut base_total = 0i64;
        let mut impuesto_total = 0i64;
        for item in &input.items {
            if item.cantidad <= 0 {
                return Err(ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad devuelta debe ser positiva.",
                ));
            }
            let linea = orden
                .items
                .iter()
                .find(|linea| linea.oferta_id == item.oferta_id)
                .ok_or_else(|| {
                    ApiError::dominio("LINEA_NO_ENCONTRADA", "La línea no pertenece a la orden.")
                })?;
            let devuelto = ya_devuelto.get(&item.oferta_id).copied().unwrap_or(0);
            if devuelto + item.cantidad > linea.cantidad {
                return Err(ApiError::dominio(
                    "CANTIDAD_EXCEDIDA",
                    "La cantidad devuelta supera lo comprado.",
                ));
            }
            let descuento = if linea.cantidad > 0 {
                ((linea.descuento_cents as f64 * item.cantidad as f64) / linea.cantidad as f64)
                    .round() as i64
            } else {
                0
            };
            let neto =
                (linea.precio_unitario_cents.saturating_mul(item.cantidad) - descuento).max(0);
            let factor = if linea.cantidad > 0 {
                item.cantidad as f64 / linea.cantidad as f64
            } else {
                0.0
            };
            let total_linea = if linea.total_linea_cents > 0 {
                (linea.total_linea_cents as f64 * factor).round() as i64
            } else {
                neto
            };
            let impuesto = if linea.impuesto_cents > 0 {
                (linea.impuesto_cents as f64 * factor).round() as i64
            } else {
                0
            };
            let base = (total_linea - impuesto).max(0);
            total += total_linea;
            base_total += base;
            impuesto_total += impuesto;
            lineas.push((
                linea,
                item.cantidad,
                descuento,
                base,
                impuesto,
                total_linea,
                item.restock,
            ));
        }
        let id = Uuid::new_v4();
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "INSERT INTO orders.devoluciones
               (id, order_id, empresa_id, negocio_id, motivo, estado,
                base_cents, impuesto_cents, total_cents, moneda, creado_por)
             VALUES ($1, $2::uuid, $3::uuid, $4::uuid, $5, 'solicitada',
                     $6, $7, $8, $9, $10::uuid)",
        )
        .bind(id)
        .bind(order_id)
        .bind(&orden.items[0].empresa_id)
        .bind(&orden.negocio_id)
        .bind(input.motivo.trim())
        .bind(base_total as i32)
        .bind(impuesto_total as i32)
        .bind(total as i32)
        .bind(&orden.moneda)
        .bind(&actor.user_id)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        for (linea, cantidad, descuento, base, impuesto, total_linea, restock) in lineas {
            sqlx::query(
                "INSERT INTO orders.devolucion_lineas
                   (devolucion_id, oferta_id, sku, cantidad, precio_unitario_cents,
                    descuento_cents, base_cents, impuesto_cents, tasa_impuesto_bps,
                    total_cents, restock, ubicacion_id)
                 VALUES ($1, $2::uuid, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::uuid)",
            )
            .bind(id)
            .bind(&linea.oferta_id)
            .bind(&linea.sku)
            .bind(cantidad as i32)
            .bind(linea.precio_unitario_cents as i32)
            .bind(descuento as i32)
            .bind(base as i32)
            .bind(impuesto as i32)
            .bind(linea.tasa_impuesto_bps as i32)
            .bind(total_linea as i32)
            .bind(restock)
            .bind(&linea.ubicacion_id)
            .execute(&mut *tx)
            .await
            .map_err(crate::event_store::error_db)?;
        }
        tx.commit().await.map_err(crate::event_store::error_db)?;
        self.obtener(&id.to_string())
            .await?
            .ok_or_else(ApiError::interno)
    }

    pub async fn listar_por_orden(
        &self,
        order_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<Devolucion>, ApiError> {
        let orden = self
            .vistas
            .encontrar(&self.db, order_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Orden", order_id))?;
        autorizar(&orden, actor)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT id, order_id, empresa_id, negocio_id, motivo, estado,
                    base_cents, impuesto_cents, total_cents, moneda,
                    reembolso_estado, reembolsado_cents, reembolso_error,
                    nota_credito_id, nota_credito_estado, nota_credito_error,
                    creado_por, creado_en, actualizado_en
               FROM orders.devoluciones WHERE order_id = $1::uuid
              ORDER BY creado_en DESC",
        )
        .bind(order_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        let mut salida = Vec::with_capacity(filas.len());
        for fila in filas {
            let id = uuid_texto(&fila, "id").map_err(crate::event_store::error_db)?;
            if let Some(devolucion) = self.obtener(&id).await? {
                salida.push(devolucion);
            }
        }
        Ok(salida)
    }

    pub async fn obtener(&self, id: &str) -> Result<Option<Devolucion>, ApiError> {
        if uuid::Uuid::parse_str(id).is_err() {
            return Ok(None);
        }
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT id, order_id, empresa_id, negocio_id, motivo, estado,
                    base_cents, impuesto_cents, total_cents, moneda,
                    reembolso_estado, reembolsado_cents, reembolso_error,
                    nota_credito_id, nota_credito_estado, nota_credito_error,
                    creado_por, creado_en, actualizado_en
               FROM orders.devoluciones WHERE id = $1::uuid",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let Some(fila) = fila else {
            tx.commit().await.map_err(crate::event_store::error_db)?;
            return Ok(None);
        };
        let lineas = sqlx::query(
            "SELECT id, oferta_id, sku, cantidad, precio_unitario_cents, descuento_cents,
                    base_cents, impuesto_cents, tasa_impuesto_bps, total_cents,
                    restock, ubicacion_id
               FROM orders.devolucion_lineas WHERE devolucion_id = $1::uuid
              ORDER BY oferta_id",
        )
        .bind(id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        Ok(Some(fila_devolucion(&fila, &lineas)?))
    }
}

fn autorizar(orden: &crate::views::OrderView, actor: &UsuarioContexto) -> Result<(), ApiError> {
    if actor.es_admin_global()
        || (actor.rol == "comprador" && orden.comprador_id == actor.user_id)
        || (actor.rol == "empresario"
            && actor.tenant_id.as_deref()
                == orden.items.first().map(|item| item.empresa_id.as_str()))
    {
        Ok(())
    } else {
        Err(ApiError::acceso_denegado(
            "No tienes acceso a las devoluciones de esta orden.",
        ))
    }
}

fn fila_devolucion(
    fila: &sqlx::postgres::PgRow,
    lineas: &[sqlx::postgres::PgRow],
) -> Result<Devolucion, ApiError> {
    let creado: DateTime<Utc> = fila
        .try_get("creado_en")
        .map_err(crate::event_store::error_db)?;
    let actualizado: DateTime<Utc> = fila
        .try_get("actualizado_en")
        .map_err(crate::event_store::error_db)?;
    Ok(Devolucion {
        id: uuid_texto(fila, "id").map_err(crate::event_store::error_db)?,
        order_id: uuid_texto(fila, "order_id").map_err(crate::event_store::error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(crate::event_store::error_db)?,
        negocio_id: crate::db::uuid_opcional(fila, "negocio_id")
            .map_err(crate::event_store::error_db)?,
        motivo: fila
            .try_get("motivo")
            .map_err(crate::event_store::error_db)?,
        estado: fila
            .try_get("estado")
            .map_err(crate::event_store::error_db)?,
        base_cents: i64::from(fila.try_get::<i32, _>("base_cents").unwrap_or(0)),
        impuesto_cents: i64::from(fila.try_get::<i32, _>("impuesto_cents").unwrap_or(0)),
        total_cents: i64::from(
            fila.try_get::<i32, _>("total_cents")
                .map_err(crate::event_store::error_db)?,
        ),
        moneda: fila
            .try_get("moneda")
            .map_err(crate::event_store::error_db)?,
        reembolso_estado: fila
            .try_get("reembolso_estado")
            .unwrap_or_else(|_| "pendiente".to_string()),
        reembolsado_cents: i64::from(fila.try_get::<i32, _>("reembolsado_cents").unwrap_or(0)),
        reembolso_error: fila.try_get("reembolso_error").unwrap_or_default(),
        nota_credito_id: uuid_opcional(fila, "nota_credito_id")
            .map_err(crate::event_store::error_db)?,
        nota_credito_estado: fila
            .try_get("nota_credito_estado")
            .unwrap_or_else(|_| "pendiente".to_string()),
        nota_credito_error: fila.try_get("nota_credito_error").unwrap_or_default(),
        creado_por: uuid_texto(fila, "creado_por").map_err(crate::event_store::error_db)?,
        creado_en: iso_ms(creado),
        actualizado_en: iso_ms(actualizado),
        lineas: lineas
            .iter()
            .map(|linea| {
                Ok(LineaDevolucion {
                    id: uuid_texto(linea, "id").map_err(crate::event_store::error_db)?,
                    oferta_id: uuid_texto(linea, "oferta_id")
                        .map_err(crate::event_store::error_db)?,
                    sku: linea.try_get("sku").map_err(crate::event_store::error_db)?,
                    cantidad: i64::from(
                        linea
                            .try_get::<i32, _>("cantidad")
                            .map_err(crate::event_store::error_db)?,
                    ),
                    precio_unitario_cents: i64::from(
                        linea
                            .try_get::<i32, _>("precio_unitario_cents")
                            .map_err(crate::event_store::error_db)?,
                    ),
                    descuento_cents: i64::from(
                        linea
                            .try_get::<i32, _>("descuento_cents")
                            .map_err(crate::event_store::error_db)?,
                    ),
                    base_cents: i64::from(linea.try_get::<i32, _>("base_cents").unwrap_or(0)),
                    impuesto_cents: i64::from(
                        linea.try_get::<i32, _>("impuesto_cents").unwrap_or(0),
                    ),
                    tasa_impuesto_bps: i64::from(
                        linea.try_get::<i32, _>("tasa_impuesto_bps").unwrap_or(0),
                    ),
                    total_cents: i64::from(
                        linea
                            .try_get::<i32, _>("total_cents")
                            .map_err(crate::event_store::error_db)?,
                    ),
                    restock: linea
                        .try_get("restock")
                        .map_err(crate::event_store::error_db)?,
                    ubicacion_id: crate::db::uuid_opcional(linea, "ubicacion_id")
                        .map_err(crate::event_store::error_db)?,
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?,
    })
}
