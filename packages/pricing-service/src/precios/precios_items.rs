use super::*;

impl Precios {
    pub async fn crear_precio(
        &self,
        empresa_id: &str,
        lista_id: Option<&str>,
        input: &Value,
    ) -> Result<Value, ApiError> {
        if let Some(lista) = lista_id {
            if !lista.is_empty() {
                self.buscar_lista(empresa_id, lista).await?;
            }
        }
        let item = self
            .verificar_item(empresa_id, &texto(input, "item_id"))
            .await?;
        let desde_por_defecto = json!(1);
        let desde = input.get("desde_cantidad").unwrap_or(&desde_por_defecto);
        let hasta = input.get("hasta_cantidad").filter(|valor| !valor.is_null());
        let Some(precio) = input.get("precio_cents") else {
            return Err(ApiError::dominio(
                "PRECIO_INVALIDO",
                "El precio debe ser un entero no negativo.",
            ));
        };
        let rango = validar_rango_cantidad(desde, hasta, precio)?;

        let lista_bd = lista_id.filter(|valor| !valor.is_empty());
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            let fila = sqlx::query(&format!(
                "INSERT INTO pricing.lista_precio_items
                   (empresa_id, lista_id, item_id, desde_cantidad, hasta_cantidad, precio_cents)
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4::bigint, $5::bigint, $6::bigint)
                 RETURNING {COLUMNAS_PRECIO}"
            ))
            .bind(empresa_id)
            .bind(lista_bd)
            .bind(&item.id)
            .bind(rango.desde)
            .bind(rango.hasta)
            .bind(rango.precio)
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        let fila = match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)?
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                return Err(ApiError::dominio(
                    "PRECIO_NO_CREADO",
                    "No se pudo guardar el precio.",
                ));
            }
            Err(error) => {
                let _ = tx.rollback().await;
                return Err(error_conflicto(error, MENSAJE_CONFLICTO_ESCALA));
            }
        };

        if lista_bd.is_none() {
            self.marcar_escala(empresa_id, &item.id).await?;
        }
        Ok(fila)
    }

    pub async fn actualizar_precio(
        &self,
        empresa_id: &str,
        lista_id: Option<&str>,
        precio_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_precio(empresa_id, lista_id, precio_id).await?;

        let desde_actual = json!(entero_de(&actual, "desde_cantidad").unwrap_or(1));
        let desde = input
            .get("desde_cantidad")
            .filter(|valor| !valor.is_null())
            .unwrap_or(&desde_actual);

        let hasta_actual = actual.get("hasta_cantidad").and_then(Value::as_i64);
        let hasta_actual_valor = hasta_actual.map(|valor| json!(valor));
        let hasta = match input.get("hasta_cantidad") {
            Some(Value::Null) => None,
            Some(valor) => Some(valor),
            None => hasta_actual_valor.as_ref(),
        };

        let precio_actual = json!(entero_de(&actual, "precio_cents").unwrap_or(0));
        let precio = input
            .get("precio_cents")
            .filter(|valor| !valor.is_null())
            .unwrap_or(&precio_actual);

        let rango = validar_rango_cantidad(desde, hasta, precio)?;
        let lista_bd = lista_id;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            let fila = sqlx::query(&format!(
                "UPDATE pricing.lista_precio_items
                    SET desde_cantidad = $1::bigint, hasta_cantidad = $2::bigint,
                        precio_cents = $3::bigint, actualizado_en = NOW()
                  WHERE id = $4::uuid AND empresa_id = $5::uuid
                    AND lista_id IS NOT DISTINCT FROM $6::uuid
                  RETURNING {COLUMNAS_PRECIO}"
            ))
            .bind(rango.desde)
            .bind(rango.hasta)
            .bind(rango.precio)
            .bind(precio_id)
            .bind(empresa_id)
            .bind(lista_bd)
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                Err(ApiError::no_encontrado("Precio", precio_id))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error, MENSAJE_CONFLICTO_ESCALA))
            }
        }
    }

    pub async fn eliminar_precio(
        &self,
        empresa_id: &str,
        lista_id: Option<&str>,
        precio_id: &str,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_precio(empresa_id, lista_id, precio_id).await?;
        let lista_bd = lista_id;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "DELETE FROM pricing.lista_precio_items
              WHERE id = $1::uuid AND empresa_id = $2::uuid
                AND lista_id IS NOT DISTINCT FROM $3::uuid
              RETURNING id",
        )
        .bind(precio_id)
        .bind(empresa_id)
        .bind(lista_bd)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if fila.is_none() {
            let _ = tx.rollback().await;
            return Err(ApiError::no_encontrado("Precio", precio_id));
        }
        tx.commit().await.map_err(interno)?;

        if lista_bd.is_none() {
            self.actualizar_tipo_precio(empresa_id, &texto_de(&actual, "item_id"))
                .await?;
        }
        Ok(json!({ "ok": true }))
    }

    /// Fija el precio base del item (fila `lista_id NULL`, `desde_cantidad 1`).
    pub async fn fijar_precio_base(
        &self,
        empresa_id: &str,
        item_id: &str,
        precio_cents: i64,
    ) -> Result<Value, ApiError> {
        if precio_cents < 0 {
            return Err(ApiError::validacion(json!([{
                "campo": "precio_cents",
                "restricciones": ["min"],
            }])));
        }
        let item = self.verificar_item(empresa_id, item_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let actual = sqlx::query(
            "SELECT id, precio_cents
               FROM pricing.lista_precio_items
              WHERE empresa_id = $1::uuid AND item_id = $2::uuid
                AND lista_id IS NULL AND desde_cantidad = 1
              FOR UPDATE",
        )
        .bind(empresa_id)
        .bind(&item.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let anterior = actual
            .as_ref()
            .map(|fila| fila.get::<i32, _>("precio_cents") as i64);
        match actual {
            Some(fila) => {
                let id: uuid::Uuid = fila.try_get("id").map_err(interno)?;
                sqlx::query(
                    "UPDATE pricing.lista_precio_items
                        SET precio_cents = $1, actualizado_en = NOW()
                      WHERE id = $2::uuid",
                )
                .bind(precio_cents as i32)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            }
            None => {
                sqlx::query(
                    "INSERT INTO pricing.lista_precio_items
                       (empresa_id, lista_id, item_id, desde_cantidad, hasta_cantidad, precio_cents)
                     VALUES ($1::uuid, NULL, $2::uuid, 1, NULL, $3)",
                )
                .bind(empresa_id)
                .bind(&item.id)
                .bind(precio_cents as i32)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            }
        }
        sqlx::query(
            "INSERT INTO pricing.historico_precios_base
               (empresa_id, item_id, precio_anterior_cents, precio_nuevo_cents)
             VALUES ($1::uuid, $2::uuid, $3, $4)",
        )
        .bind(empresa_id)
        .bind(&item.id)
        .bind(anterior)
        .bind(precio_cents as i32)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        self.actualizar_tipo_precio(empresa_id, &item.id).await?;
        Ok(json!({
            "item_id": item.id,
            "precio_cents": precio_cents,
        }))
    }

    // ── escalas base del item ───────────────────────────────────────────────

    pub(super) async fn buscar_precio(
        &self,
        empresa_id: &str,
        lista_id: Option<&str>,
        precio_id: &str,
    ) -> Result<Value, ApiError> {
        let id = validar_uuid(precio_id, "precio")?;
        let lista_bd = lista_id;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_PRECIO}
               FROM pricing.lista_precio_items
              WHERE id = $1::uuid AND empresa_id = $2::uuid
                AND lista_id IS NOT DISTINCT FROM $3::uuid"
        ))
        .bind(&id)
        .bind(empresa_id)
        .bind(lista_bd)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_json(&fila),
            None => Err(ApiError::no_encontrado("Precio", &id)),
        }
    }

    async fn actualizar_tipo_precio(
        &self,
        empresa_id: &str,
        item_id: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let tipo: String = sqlx::query_scalar(
            "SELECT CASE WHEN EXISTS (
                      SELECT 1 FROM pricing.lista_precio_items p
                       WHERE p.empresa_id = $1::uuid AND p.item_id = $2::uuid
                         AND p.lista_id IS NULL AND p.desde_cantidad > 1
                    ) THEN 'escala' ELSE 'fijo' END",
        )
        .bind(empresa_id)
        .bind(item_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        // R-MS-16/ADR-0051: item es dueño de su flag; la función corre en su
        // esquema y en la misma transacción.
        sqlx::query_scalar::<_, i32>("SELECT item.marcar_tipo_precio($1::uuid, $2::uuid, $3)")
            .bind(empresa_id)
            .bind(item_id)
            .bind(&tipo)
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }
}
