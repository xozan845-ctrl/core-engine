use super::*;

impl Precios {
    /// Resuelve el precio unitario y total de un item para una cantidad.
    ///
    /// `filtros` admite `lista_codigo` y/o `segmento_codigo`; sin filtros se usa
    /// la lista predeterminada activa y, si no hay, la escala base del item.
    pub async fn precio_de(
        &self,
        empresa_id: &str,
        item_id: &str,
        cantidad: &Value,
        filtros: &Value,
    ) -> Result<Value, ApiError> {
        let cantidad = validar_cantidad(cantidad)?;
        let item = self.verificar_item(empresa_id, item_id).await?;
        let lista = self.resolver_lista(empresa_id, filtros).await?;
        let lista_id = lista
            .as_ref()
            .and_then(|valor| valor.get("id"))
            .and_then(Value::as_str);
        let escala = self
            .buscar_escala(empresa_id, &item.id, lista_id, cantidad)
            .await?;
        let origen = if lista.is_some() {
            "lista"
        } else if escala
            .as_ref()
            .and_then(|valor| entero_de(valor, "desde_cantidad"))
            == Some(1)
        {
            "base"
        } else if escala.is_some() {
            "escala"
        } else {
            "base"
        };
        let precio_unitario = escala
            .as_ref()
            .and_then(|valor| entero_de(valor, "precio_cents"))
            .unwrap_or(0);
        let precio_total = multiplicar(precio_unitario, cantidad)?;

        Ok(json!({
            "item_id": item.id,
            "cantidad": cantidad,
            "lista": lista,
            "origen": origen,
            "escala": escala,
            "precio_unitario_cents": precio_unitario,
            "precio_total_cents": precio_total,
        }))
    }

    /// Devuelve la primera promoción vigente y aplicable a la línea. La
    /// decisión de cuánto descontar vive en orders, que conoce el precio final
    /// (base + margen de la oferta).
    async fn promocion_vigente(
        &self,
        empresa_id: &str,
        item_id: &str,
        sku: &str,
        cantidad: i64,
    ) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT id, nombre, tipo, cantidad_requerida, cantidad_gratis,
                    descuento_porcentaje
               FROM pricing.promociones
              WHERE empresa_id = $1::uuid AND estado = 'activo'
                AND eliminado_en IS NULL
                AND fecha_desde <= CURRENT_DATE AND fecha_hasta >= CURRENT_DATE
                AND (item_id = $2::uuid OR ($3 <> '' AND upper(sku) = upper($3)))
              ORDER BY creado_en DESC, id",
        )
        .bind(empresa_id)
        .bind(item_id)
        .bind(sku)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        for fila in filas {
            let tipo: String = fila.try_get("tipo").map_err(interno)?;
            let requerida: i64 = fila
                .try_get::<Option<i32>, _>("cantidad_requerida")
                .map_err(interno)?
                .map(i64::from)
                .unwrap_or(match tipo.as_str() {
                    "2x1" => 2,
                    _ => 1,
                });
            if cantidad < requerida {
                continue;
            }
            return Ok(Some(json!({
                "id": fila
                    .try_get::<uuid::Uuid, _>("id")
                    .map(|id| id.to_string())
                    .map_err(interno)?,
                "nombre": fila.try_get::<String, _>("nombre").map_err(interno)?,
                "tipo": tipo,
                "cantidad_requerida": requerida,
                "cantidad_gratis": fila
                    .try_get::<Option<i32>, _>("cantidad_gratis")
                    .map_err(interno)?
                    .map(i64::from),
                "descuento_porcentaje": fila
                    .try_get::<Option<i32>, _>("descuento_porcentaje")
                    .map_err(interno)?,
            })));
        }
        Ok(None)
    }

    /// Resuelve precios por lote con el shape del endpoint interno:
    /// `{empresa_id, lista_codigo?, segmento_codigo?, lineas:[{clave,item_id,cantidad}]}`.
    pub async fn precios_lote(&self, input: &Value) -> Result<Value, ApiError> {
        let empresa = input
            .get("empresa_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !es_uuid(&empresa) {
            return Err(ApiError::dominio(
                "EMPRESA_REQUERIDA",
                "Indica una empresa válida.",
            ));
        }
        let lineas = input
            .get("lineas")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ApiError::validacion(json!([
                    {"campo": "lineas", "restricciones": ["isArray", "arrayMinSize"]}
                ]))
            })?;
        if lineas.is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "lineas", "restricciones": ["arrayMinSize"]}
            ])));
        }

        let filtros = json!({
            "lista_codigo": input.get("lista_codigo").cloned().unwrap_or(Value::Null),
            "segmento_codigo": input.get("segmento_codigo").cloned().unwrap_or(Value::Null),
        });
        let mut precios = Vec::with_capacity(lineas.len());
        for linea in lineas {
            let clave = linea.get("clave").map(texto_js).unwrap_or_default();
            let item_id = linea.get("item_id").map(texto_js).unwrap_or_default();
            let sku = linea.get("sku").map(texto_js).unwrap_or_default();
            let cantidad = linea.get("cantidad").cloned().unwrap_or(Value::Null);
            let resuelto = self
                .precio_de(&empresa, &item_id, &cantidad, &filtros)
                .await?;
            let cantidad_resuelta = resuelto
                .get("cantidad")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            let promocion = self
                .promocion_vigente(&empresa, &item_id, &sku, cantidad_resuelta)
                .await?;
            precios.push(json!({
                "clave": clave,
                "origen": resuelto.get("origen").cloned().unwrap_or(Value::Null),
                "precio_unitario_cents": resuelto
                    .get("precio_unitario_cents")
                    .cloned()
                    .unwrap_or(Value::Null),
                "precio_total_cents": resuelto
                    .get("precio_total_cents")
                    .cloned()
                    .unwrap_or(Value::Null),
                "promocion": promocion,
            }));
        }
        Ok(json!({ "precios": precios }))
    }

    pub(super) async fn verificar_item(
        &self,
        empresa_id: &str,
        item_id: &str,
    ) -> Result<ItemPrecio, ApiError> {
        let id = validar_uuid(item_id, "item")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, tipo_precio
               FROM item.items
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(&id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let fila = fila.ok_or_else(|| ApiError::no_encontrado("Item", &id))?;
        let valor = fila_json(&fila)?;
        Ok(ItemPrecio {
            id: texto_de(&valor, "id"),
            tipo_precio: texto_de(&valor, "tipo_precio"),
        })
    }
}
