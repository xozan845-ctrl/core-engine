use super::*;

impl Operaciones {
    pub(super) async fn resolver_productos(
        &self,
        empresa_id: &str,
        lineas: &[Value],
    ) -> Result<Vec<ItemResuelto>, ApiError> {
        let mut resueltos = Vec::with_capacity(lineas.len());
        for linea in lineas {
            resueltos.push(
                self.buscar_item(
                    empresa_id,
                    linea.get("item_id").and_then(Value::as_str),
                    linea.get("sku").and_then(Value::as_str).unwrap_or(""),
                )
                .await?,
            );
        }
        Ok(resueltos)
    }

    pub(super) async fn buscar_item(
        &self,
        empresa_id: &str,
        item_id: Option<&str>,
        sku: &str,
    ) -> Result<ItemResuelto, ApiError> {
        let item_id = item_id.filter(|valor| !valor.is_empty());
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match item_id {
            Some(item_id) => sqlx::query(
                "SELECT id, codigo AS sku, nombre, costo_cents, stock, es_combo
                   FROM catalog.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto' AND id = $2::uuid
                    AND eliminado_en IS NULL",
            )
            .bind(empresa_id)
            .bind(item_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => sqlx::query(
                "SELECT id, codigo AS sku, nombre, costo_cents, stock, es_combo
                   FROM catalog.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto'
                    AND upper(codigo) = upper($2) AND eliminado_en IS NULL",
            )
            .bind(empresa_id)
            .bind(sku)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
        };
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::no_encontrado("Producto", item_id.unwrap_or(sku)));
        };
        let id: Uuid = fila.try_get("id").map_err(interno)?;
        let costo_cents: Option<i32> = fila.try_get("costo_cents").map_err(interno)?;
        let stock: Option<i32> = fila.try_get("stock").map_err(interno)?;
        Ok(ItemResuelto {
            id: id.to_string(),
            sku: fila.try_get("sku").map_err(interno)?,
            nombre: fila.try_get("nombre").map_err(interno)?,
            costo_cents: costo_cents.unwrap_or(0) as i64,
            stock: stock.unwrap_or(0) as i64,
            es_combo: fila.try_get("es_combo").map_err(interno)?,
        })
    }
}
