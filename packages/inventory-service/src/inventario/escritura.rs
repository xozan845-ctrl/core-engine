use super::*;

impl Inventario {
    pub async fn crear_proveedor(&self, datos: &Value) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO inventory.proveedores
               (empresa_id, negocio_id, codigo, nombre, correo, telefono)
             VALUES ($1::uuid, $2::uuid, UPPER($3), $4, $5, $6)
             RETURNING id, empresa_id, negocio_id, codigo, nombre, correo, telefono, activo",
        )
        .bind(texto(datos, "empresa_id"))
        .bind(
            datos
                .get("negocio_id")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty()),
        )
        .bind(texto(datos, "codigo"))
        .bind(texto(datos, "nombre"))
        .bind(texto(datos, "correo"))
        .bind(texto(datos, "telefono"))
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Err(ApiError::dominio(
                "PROVEEDOR_NO_CREADO",
                "No se pudo crear el proveedor.",
            )),
        }
    }

    pub async fn crear_lote(&self, datos: &Value) -> Result<Value, ApiError> {
        let empresa_id = texto(datos, "empresa_id");
        let item_id = texto(datos, "item_id");
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let item = sqlx::query(
            "SELECT track_inventory FROM item.items
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND estado = 'activo'",
        )
        .bind(&item_id)
        .bind(&empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(item) = item else {
            return Err(ApiError::dominio(
                "ITEM_NO_ENCONTRADO",
                "El item no existe para esta empresa.",
            ));
        };
        if !item.get::<bool, _>("track_inventory") {
            return Err(ApiError::dominio(
                "INVENTARIO_NO_APLICA",
                "Los lotes solo aplican a items con inventario.",
            ));
        }
        let cantidad = datos
            .get("cantidad_inicial")
            .and_then(entero)
            .filter(|valor| *valor >= 0)
            .ok_or_else(|| {
                ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad inicial debe ser un entero mayor o igual a cero.",
                )
            })?;
        let cantidad = i32::try_from(cantidad).map_err(|_| ApiError::interno())?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO inventory.lotes_inventario
               (empresa_id, item_id, proveedor_id, codigo_lote, vence_en, cantidad_inicial, cantidad_disponible)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5::date, $6, $6)
             RETURNING id, empresa_id, item_id, proveedor_id, codigo_lote, vence_en,
                       cantidad_inicial, cantidad_disponible",
        )
        .bind(&empresa_id)
        .bind(&item_id)
        .bind(datos.get("proveedor_id").and_then(Value::as_str))
        .bind(texto(datos, "codigo_lote"))
        .bind(datos.get("vence_en").and_then(Value::as_str))
        .bind(cantidad)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => lote_a_json(&fila),
            None => Err(ApiError::dominio(
                "LOTE_NO_CREADO",
                "No se pudo crear el lote.",
            )),
        }
    }

    /// Consume cantidades aplicando FEFO (primero vence, primero sale).
    pub async fn consumir_lote(&self, datos: &Value) -> Result<Value, ApiError> {
        let cantidad = datos
            .get("cantidad")
            .and_then(entero)
            .filter(|valor| *valor > 0)
            .ok_or_else(|| {
                ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad a consumir debe ser un entero mayor que cero.",
                )
            })?;
        let empresa_id = texto(datos, "empresa_id");
        let item_id = texto(datos, "item_id");
        let solicitado = match datos.get("codigo_lote") {
            None | Some(Value::Null) => false,
            Some(Value::String(valor)) => !valor.is_empty(),
            Some(Value::Bool(valor)) => *valor,
            Some(Value::Number(valor)) => {
                valor.as_f64().map(|numero| numero != 0.0).unwrap_or(true)
            }
            Some(_) => true,
        };
        let codigo = datos
            .get("codigo_lote")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty());

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let item = sqlx::query(
            "SELECT track_inventory, codigo FROM item.items
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND estado = 'activo'
              FOR UPDATE",
        )
        .bind(&item_id)
        .bind(&empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(item) = item else {
            return Err(ApiError::dominio(
                "ITEM_NO_ENCONTRADO",
                "El item no existe para esta empresa.",
            ));
        };
        if !item.get::<bool, _>("track_inventory") {
            return Err(ApiError::dominio(
                "INVENTARIO_NO_APLICA",
                "Los lotes solo aplican a items con inventario.",
            ));
        }

        let filas = sqlx::query(
            "SELECT id, codigo_lote, cantidad_disponible,
                    (vence_en IS NOT NULL AND vence_en < CURRENT_DATE) AS vencido
               FROM inventory.lotes_inventario
              WHERE empresa_id = $1::uuid AND item_id = $2::uuid AND cantidad_disponible > 0
                AND ($3::text IS NULL OR codigo_lote = $3)
              ORDER BY vence_en NULLS LAST, creado_en, id
              FOR UPDATE",
        )
        .bind(&empresa_id)
        .bind(&item_id)
        .bind(codigo)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;

        let lotes = filas
            .iter()
            .map(|fila| LoteDisponible {
                id: fila.get::<Uuid, _>("id").to_string(),
                codigo_lote: fila.get("codigo_lote"),
                cantidad_disponible: i64::from(fila.get::<i32, _>("cantidad_disponible")),
                vencido: fila.get("vencido"),
            })
            .collect::<Vec<_>>();

        if solicitado && lotes.iter().any(|lote| lote.vencido) {
            return Err(ApiError::dominio(
                "LOTE_VENCIDO",
                "El lote solicitado está vencido.",
            ));
        }

        let plan = plan_consumo(&lotes, cantidad).map_err(|faltante| {
            ApiError::dominio(
                "STOCK_LOTE_INSUFICIENTE",
                format!("No hay cantidad suficiente en lotes vigentes; faltan {faltante}."),
            )
        })?;

        for consumo in &plan {
            let consumir = i32::try_from(consumo.cantidad).map_err(|_| ApiError::interno())?;
            sqlx::query(
                "UPDATE inventory.lotes_inventario
                    SET cantidad_disponible = cantidad_disponible - $1,
                        actualizado_en = NOW()
                  WHERE id = $2::uuid",
            )
            .bind(consumir)
            .bind(&consumo.lote_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        }

        let stock =
            crate::stock::aplicar_delta_en_transaccion(&mut tx, &empresa_id, &item_id, -cantidad)
                .await
                .map_err(|error| {
                    if error.codigo == "STOCK_INSUFICIENTE" {
                        ApiError::dominio(
                            "STOCK_INSUFICIENTE",
                            "El stock del item no alcanza para el consumo.",
                        )
                    } else {
                        error
                    }
                })?;
        if let Some(outbox) = &self.outbox {
            outbox
                .insertar_stock_updated(
                    &mut tx,
                    &format!("consumo-lote-{item_id}"),
                    "consumo_lote",
                    vec![json!({
                        "empresa_id": empresa_id,
                        "sku": item.get::<String, _>("codigo"),
                        "stock_restante": stock,
                    })],
                )
                .await?;
        }
        tx.commit().await.map_err(interno)?;

        let consumidos = plan
            .iter()
            .map(|consumo| {
                json!({
                    "lote_id": consumo.lote_id,
                    "codigo_lote": consumo.codigo_lote,
                    "cantidad": consumo.cantidad,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "consumidos": consumidos,
            "total_consumido": cantidad,
        }))
    }
}
