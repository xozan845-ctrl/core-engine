use super::prelude::*;
use super::*;

impl Devoluciones {
    pub async fn aprobar(&self, id: &str, actor: &UsuarioContexto) -> Result<Devolucion, ApiError> {
        self.cambiar_estado(id, "aprobada", actor).await
    }

    pub async fn rechazar(
        &self,
        id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Devolucion, ApiError> {
        self.cambiar_estado(id, "rechazada", actor).await
    }

    pub async fn recibir(&self, id: &str, actor: &UsuarioContexto) -> Result<Devolucion, ApiError> {
        let devolucion = self
            .obtener(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Devolucion", id))?;
        if !matches!(devolucion.estado.as_str(), "solicitada" | "aprobada") {
            return Err(ApiError::conflicto(format!(
                "La devolución no puede recibirse en estado {}.",
                devolucion.estado
            )));
        }
        let items: Vec<Value> = devolucion
            .lineas
            .iter()
            .filter(|linea| linea.restock)
            .map(|linea| {
                json!({
                    "empresa_id": devolucion.empresa_id,
                    "sku": linea.sku,
                    "cantidad": linea.cantidad,
                    "ubicacion_id": linea.ubicacion_id,
                })
            })
            .collect();
        if !items.is_empty()
            && !self
                .clientes
                .devolver_stock(&devolucion.order_id, id, &items)
                .await
        {
            return Err(ApiError::servicio_no_disponible("inventory-service"));
        }
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "UPDATE orders.devoluciones SET estado = 'recibida', actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let evento = json!({
            "event_id": Uuid::new_v4().to_string(),
            "tipo": "devolucion.recibida",
            "ocurrido_en": iso_ms(Utc::now()),
            "data": {
                "order_id": devolucion.order_id,
                "devolucion_id": devolucion.id,
                "actor_id": actor.user_id,
                "empresa_id": devolucion.empresa_id,
                "items": devolucion.lineas.iter().map(|linea| json!({
                    "empresa_id": devolucion.empresa_id,
                    "linea_clave": linea.oferta_id,
                    "sku": linea.sku,
                    "cantidad": linea.cantidad,
                    "base_cents": linea.base_cents,
                    "impuesto_cents": linea.impuesto_cents,
                    "monto_cents": linea.total_cents,
                })).collect::<Vec<_>>(),
            }
        });
        let sql = format!(
            "INSERT INTO {} (event_id, tipo, payload, estado, creado_en)
             VALUES ($1::uuid, $2, $3::jsonb, 'pendiente', NOW())",
            self.outbox.tabla()
        );
        sqlx::query(&sql)
            .bind(evento["event_id"].as_str().unwrap_or_default())
            .bind("devolucion.recibida")
            .bind(evento.to_string())
            .execute(&mut *tx)
            .await
            .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        // El reembolso y la nota de crédito son efectos posteriores: si fallan,
        // la devolución queda recibida y se pueden reintentar por API.
        let _ = self.reembolsar_devolucion(id, actor).await;
        let _ = self.emitir_nota_credito(id, actor).await;
        self.obtener(id).await?.ok_or_else(ApiError::interno)
    }

    async fn cambiar_estado(
        &self,
        id: &str,
        estado: &str,
        actor: &UsuarioContexto,
    ) -> Result<Devolucion, ApiError> {
        if !actor.es_admin_global() {
            return Err(ApiError::acceso_denegado(
                "Solo un administrador puede cambiar el estado de la devolución.",
            ));
        }
        let actual = self
            .obtener(id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Devolucion", id))?;
        if actual.estado != "solicitada" {
            return Err(ApiError::conflicto("La devolución ya fue procesada."));
        }
        sqlx::query(
            "UPDATE orders.devoluciones SET estado = $2, actualizado_en = NOW()
              WHERE id = $1::uuid",
        )
        .bind(id)
        .bind(estado)
        .execute(self.db.pool())
        .await
        .map_err(crate::event_store::error_db)?;
        self.obtener(id).await?.ok_or_else(ApiError::interno)
    }

    pub(super) async fn cantidades_devueltas(
        &self,
        order_id: &str,
    ) -> Result<std::collections::HashMap<String, i64>, ApiError> {
        let filas = sqlx::query(
            "SELECT l.oferta_id::text AS oferta_id, SUM(l.cantidad)::bigint AS cantidad
               FROM orders.devolucion_lineas l
               JOIN orders.devoluciones d ON d.id = l.devolucion_id
              WHERE d.order_id = $1::uuid AND d.estado <> 'rechazada'
              GROUP BY l.oferta_id",
        )
        .bind(order_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(crate::event_store::error_db)?;
        let mut salida = std::collections::HashMap::new();
        for fila in filas {
            salida.insert(
                fila.try_get::<String, _>("oferta_id")
                    .map_err(crate::event_store::error_db)?,
                fila.try_get::<i64, _>("cantidad")
                    .map_err(crate::event_store::error_db)?,
            );
        }
        Ok(salida)
    }
}
