use super::*;

impl Precios {
    pub async fn escalas(&self, empresa_id: &str, item_id: &str) -> Result<Value, ApiError> {
        let item = self.verificar_item(empresa_id, item_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_PRECIO}
               FROM pricing.lista_precio_items
              WHERE empresa_id = $1::uuid AND item_id = $2::uuid AND lista_id IS NULL
              ORDER BY desde_cantidad"
        ))
        .bind(empresa_id)
        .bind(&item.id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas_json(&filas)
    }

    pub async fn crear_escala(
        &self,
        empresa_id: &str,
        item_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let mut entrada = Map::new();
        if let Value::Object(objeto) = input {
            entrada = objeto.clone();
        }
        entrada.insert("item_id".into(), Value::String(item_id.to_string()));
        self.crear_precio(empresa_id, None, &Value::Object(entrada))
            .await
    }

    pub async fn actualizar_escala(
        &self,
        empresa_id: &str,
        item_id: &str,
        escala_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_precio(empresa_id, None, escala_id).await?;
        if texto_de(&actual, "item_id") != item_id {
            return Err(ApiError::no_encontrado("Escala", escala_id));
        }
        self.actualizar_precio(empresa_id, None, escala_id, input)
            .await
    }

    pub async fn eliminar_escala(
        &self,
        empresa_id: &str,
        item_id: &str,
        escala_id: &str,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_precio(empresa_id, None, escala_id).await?;
        if texto_de(&actual, "item_id") != item_id {
            return Err(ApiError::no_encontrado("Escala", escala_id));
        }
        self.eliminar_precio(empresa_id, None, escala_id).await
    }

    // ── resolución ──────────────────────────────────────────────────────────

    pub(super) async fn buscar_escala(
        &self,
        empresa_id: &str,
        item_id: &str,
        lista_id: Option<&str>,
        cantidad: i64,
    ) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_PRECIO}
               FROM pricing.lista_precio_items
              WHERE empresa_id = $1::uuid AND item_id = $2::uuid
                AND lista_id IS NOT DISTINCT FROM $3::uuid
                AND desde_cantidad <= $4::bigint
                AND (hasta_cantidad IS NULL OR hasta_cantidad >= $4::bigint)
              ORDER BY desde_cantidad DESC
              LIMIT 1"
        ))
        .bind(empresa_id)
        .bind(item_id)
        .bind(lista_id)
        .bind(cantidad)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(Some(fila_json(&fila)?)),
            None => Ok(None),
        }
    }

    // ── helpers ─────────────────────────────────────────────────────────────

    pub(super) async fn marcar_escala(
        &self,
        empresa_id: &str,
        item_id: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "UPDATE item.items
                SET tipo_precio = 'escala', actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND tipo_precio <> 'escala'",
        )
        .bind(item_id)
        .bind(empresa_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }
}
