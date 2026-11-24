use super::*;

impl Operaciones {
    pub async fn listar_combos(
        &self,
        empresa_id: &str,
        filtros: &Value,
    ) -> Result<Value, ApiError> {
        let pagina = filtro_texto(filtros, "pagina");
        let limite = filtro_texto(filtros, "limite");
        let paginacion = parsear_paginacion(pagina.as_deref(), limite.as_deref());
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        let mut condiciones = vec![
            "i.empresa_id = $1::uuid".to_string(),
            "i.tipo = 'producto'".to_string(),
            "i.es_combo".to_string(),
            "i.eliminado_en IS NULL".to_string(),
        ];
        if let Some(q) = filtro_texto(filtros, "q").filter(|valor| !valor.is_empty()) {
            params.push(format!("%{}%", q.to_lowercase()));
            condiciones.push(format!(
                "(LOWER(i.nombre) LIKE ${0} OR LOWER(i.codigo) LIKE ${0}
                  OR LOWER(i.codigo_barras) LIKE ${0})",
                params.len()
            ));
        }
        if let Some(estado) = filtro_texto(filtros, "estado").filter(|valor| !valor.is_empty()) {
            params.push(estado);
            condiciones.push(format!("i.estado = ${}", params.len()));
        }
        let where_clause = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total = format!("SELECT COUNT(*)::int AS n FROM item.items i {where_clause}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");

        let sql = format!(
            "{SELECT_COMBO}
             {where_clause}
             GROUP BY i.id
             ORDER BY i.creado_en DESC
             LIMIT ${} OFFSET ${}",
            params.len() + 1,
            params.len() + 2
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta
            .bind(paginacion.limite)
            .bind((paginacion.pagina - 1) * paginacion.limite)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let mut filas_json = Vec::with_capacity(filas.len());
        for fila in &filas {
            filas_json.push(fila_a_json(fila).map_err(interno)?);
        }
        let combos = self.hidratar_combos(filas_json).await?;
        Ok(crear_pagina(combos, total as i64, paginacion))
    }

    pub async fn obtener_combo(
        &self,
        id: &str,
        empresa_id: &str,
    ) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "{SELECT_COMBO}
             WHERE i.id = $1::uuid AND i.empresa_id = $2::uuid
               AND i.tipo = 'producto' AND i.es_combo AND i.eliminado_en IS NULL
             GROUP BY i.id"
        ))
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(None);
        };
        let valor = fila_a_json(&fila).map_err(interno)?;
        Ok(self.hidratar_combos(vec![valor]).await?.into_iter().next())
    }

    pub async fn crear_combo(&self, empresa_id: &str, datos: &Value) -> Result<Value, ApiError> {
        let sku = normalizar_sku(datos.get("sku").and_then(Value::as_str).unwrap_or(""))?;
        let nombre = validar_nombre(&texto(datos, "nombre"))?;
        let precio = monto_de(datos.get("precio"))?;
        let componentes = self
            .resolver_componentes(empresa_id, datos.get("componentes"))
            .await?;
        let codigo_barras = texto_opcional(datos, "codigo_barras").unwrap_or_default();
        self.validar_combo_unico(empresa_id, &sku, &codigo_barras, None)
            .await?;

        let id = Uuid::new_v4().to_string();
        let descripcion = texto_opcional(datos, "descripcion").unwrap_or_default();
        let estado = estado_operativo(datos);
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let ejecucion: Result<(), sqlx::Error> = async {
            sqlx::query(
                "INSERT INTO item.items
                   (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
                    costo_cents, track_inventory, atributos, estado,
                    codigo_barras, es_combo)
                 VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, 'combo', 0, false,
                         '{}'::jsonb, $6, $7, true)",
            )
            .bind(&id)
            .bind(empresa_id)
            .bind(&sku)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(&estado)
            .bind(&codigo_barras)
            .execute(&mut *tx)
            .await?;
            for componente in &componentes {
                sqlx::query(
                    "INSERT INTO item.item_componentes
                       (empresa_id, item_id, componente_item_id, cantidad)
                     VALUES ($1::uuid, $2::uuid, $3::uuid, $4)",
                )
                .bind(empresa_id)
                .bind(&id)
                .bind(&componente.item_id)
                .bind(componente.cantidad)
                .execute(&mut *tx)
                .await?;
            }
            Ok(())
        }
        .await;
        match ejecucion {
            Ok(()) => tx.commit().await.map_err(interno)?,
            Err(error) => {
                let _ = tx.rollback().await;
                if es_violacion_unicidad(&error) {
                    return Err(ApiError::conflicto(format!(
                        "Ya existe un item con el SKU {sku}."
                    )));
                }
                return Err(interno(error));
            }
        }
        if let Some(proyecciones) = &self.proyecciones {
            if let Err(error) = proyecciones
                .fijar_precio(empresa_id, &id, precio.centavos)
                .await
            {
                let mut tx = self.db.transaccion().await.map_err(interno)?;
                sqlx::query("DELETE FROM item.items WHERE id = $1::uuid")
                    .bind(&id)
                    .execute(&mut *tx)
                    .await
                    .map_err(interno)?;
                tx.commit().await.map_err(interno)?;
                return Err(error);
            }
        }
        self.obtener_combo(&id, empresa_id)
            .await?
            .ok_or_else(ApiError::interno)
    }

    pub async fn actualizar_combo(
        &self,
        id: &str,
        empresa_id: &str,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self
            .obtener_combo(id, empresa_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Combo", id))?;

        let componentes_input = match datos.get("componentes") {
            Some(valor) => valor.clone(),
            None => Value::Array(
                actual
                    .get("componentes")
                    .and_then(Value::as_array)
                    .map(|componentes| {
                        componentes
                            .iter()
                            .map(|componente| {
                                json!({
                                    "item_id": componente.get("item_id"),
                                    "sku": componente.get("sku"),
                                    "cantidad": componente.get("cantidad"),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            ),
        };
        let componentes = self
            .resolver_componentes(empresa_id, Some(&componentes_input))
            .await?;

        let codigo_actual = actual
            .get("codigo_barras")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let codigo_barras = match datos.get("codigo_barras") {
            Some(_) => texto_opcional(datos, "codigo_barras").unwrap_or_default(),
            None => codigo_actual.clone(),
        };
        if codigo_barras != codigo_actual {
            self.validar_combo_unico(empresa_id, actual_sku(&actual), &codigo_barras, Some(id))
                .await?;
        }

        let nombre = match datos.get("nombre") {
            Some(_) => validar_nombre(&texto(datos, "nombre"))?,
            None => texto(&actual, "nombre"),
        };
        let descripcion = match datos.get("descripcion") {
            Some(_) => texto_opcional(datos, "descripcion").unwrap_or_default(),
            None => texto(&actual, "descripcion"),
        };
        let nuevo_precio = datos
            .get("precio")
            .map(|_| monto_de(datos.get("precio")))
            .transpose()?
            .map(|monto| monto.centavos);
        let estado = match datos.get("estado") {
            Some(_) => estado_operativo(datos),
            None => texto(&actual, "estado"),
        };

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let ejecucion: Result<(), sqlx::Error> = async {
            let actualizado = sqlx::query(
                "UPDATE item.items
                    SET nombre = $3, descripcion = $4, codigo_barras = $5,
                        estado = $6, actualizado_en = NOW()
                  WHERE id = $1::uuid AND empresa_id = $2::uuid
                    AND tipo = 'producto' AND es_combo AND eliminado_en IS NULL",
            )
            .bind(id)
            .bind(empresa_id)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(&codigo_barras)
            .bind(&estado)
            .execute(&mut *tx)
            .await?;
            if actualizado.rows_affected() == 0 {
                return Err(sqlx::Error::RowNotFound);
            }
            sqlx::query(
                "DELETE FROM item.item_componentes WHERE item_id = $1::uuid AND empresa_id = $2::uuid",
            )
            .bind(id)
            .bind(empresa_id)
            .execute(&mut *tx)
            .await?;
            for componente in &componentes {
                sqlx::query(
                    "INSERT INTO item.item_componentes
                       (empresa_id, item_id, componente_item_id, cantidad)
                     VALUES ($1::uuid, $2::uuid, $3::uuid, $4)",
                )
                .bind(empresa_id)
                .bind(id)
                .bind(&componente.item_id)
                .bind(componente.cantidad)
                .execute(&mut *tx)
                .await?;
            }
            Ok(())
        }
        .await;
        match ejecucion {
            Ok(()) => tx.commit().await.map_err(interno)?,
            Err(sqlx::Error::RowNotFound) => {
                let _ = tx.rollback().await;
                return Err(ApiError::no_encontrado("Combo", id));
            }
            Err(error) => {
                let _ = tx.rollback().await;
                if es_violacion_unicidad(&error) {
                    return Err(ApiError::conflicto(format!(
                        "El código de barras {codigo_barras} ya está registrado."
                    )));
                }
                return Err(interno(error));
            }
        }
        if let (Some(precio_cents), Some(proyecciones)) = (nuevo_precio, &self.proyecciones) {
            proyecciones
                .fijar_precio(empresa_id, id, precio_cents)
                .await?;
        }
        self.obtener_combo(id, empresa_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Combo", id))
    }

    pub async fn eliminar_combo(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE item.items SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid
                AND tipo = 'producto' AND es_combo AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Combo", id));
        }
        Ok(())
    }

    // ── helpers ──────────────────────────────────────────────────────────

    async fn hidratar_combos(&self, filas: Vec<Value>) -> Result<Vec<Value>, ApiError> {
        let mut ids: Vec<String> = Vec::new();
        for fila in &filas {
            let componentes = fila
                .get("componentes")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for componente in componentes {
                if let Some(item_id) = componente.get("item_id").and_then(Value::as_str) {
                    if !ids.iter().any(|existente| existente == item_id) {
                        ids.push(item_id.to_string());
                    }
                }
            }
        }

        let mut items: HashMap<String, (i64, String)> = HashMap::new();
        let uuids: Vec<Uuid> = ids
            .iter()
            .filter_map(|id| Uuid::parse_str(id).ok())
            .collect();
        if !uuids.is_empty() {
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let filas_items = sqlx::query(
                "SELECT id, costo_cents, nombre FROM item.items
                  WHERE id = ANY($1::uuid[]) AND eliminado_en IS NULL",
            )
            .bind(&uuids)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
            tx.commit().await.map_err(interno)?;
            for fila in filas_items {
                let id: Uuid = fila.try_get("id").map_err(interno)?;
                let costo: Option<i32> = fila.try_get("costo_cents").map_err(interno)?;
                let nombre: Option<String> = fila.try_get("nombre").map_err(interno)?;
                items.insert(
                    id.to_string(),
                    (costo.unwrap_or(0) as i64, nombre.unwrap_or_default()),
                );
            }
        }

        let mut resultado = Vec::with_capacity(filas.len());
        for fila in &filas {
            let componentes_fila = fila
                .get("componentes")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let mut componentes = Vec::with_capacity(componentes_fila.len());
            let mut costo_total: i64 = 0;
            for componente in componentes_fila {
                let item_id = componente
                    .get("item_id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                let cantidad = componente
                    .get("cantidad")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let (costo, nombre_item) = items
                    .get(&item_id)
                    .cloned()
                    .unwrap_or_else(|| (0, texto(componente, "nombre")));
                let subtotal = costo.saturating_mul(cantidad);
                costo_total = costo_total.saturating_add(subtotal);
                componentes.push(json!({
                    "item_id": item_id,
                    "sku": componente.get("sku").cloned().unwrap_or(Value::Null),
                    "nombre": nombre_item,
                    "cantidad": cantidad,
                    "costo_unitario": Money::desde_centavos(costo)?.string(),
                    "subtotal": Money::desde_centavos(subtotal)?.string(),
                }));
            }
            resultado.push(json!({
                "id": fila.get("id").cloned().unwrap_or(Value::Null),
                "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
                "sku": fila.get("sku").cloned().unwrap_or(Value::Null),
                "nombre": fila.get("nombre").cloned().unwrap_or(Value::Null),
                "descripcion": fila.get("descripcion").cloned().unwrap_or(Value::Null),
                "codigo_barras": fila.get("codigo_barras").cloned().unwrap_or(Value::Null),
                "costo_total": Money::desde_centavos(costo_total)?.string(),
                "estado": estado_serializado(fila.get("estado")),
                "componentes": componentes,
                "creado_en": fila.get("creado_en").cloned().unwrap_or(Value::Null),
                "actualizado_en": fila.get("actualizado_en").cloned().unwrap_or(Value::Null),
            }));
        }
        Ok(resultado)
    }

    async fn resolver_componentes(
        &self,
        empresa_id: &str,
        componentes: Option<&Value>,
    ) -> Result<Vec<ComponenteResuelto>, ApiError> {
        let Some(componentes) = componentes.and_then(Value::as_array) else {
            return Err(ApiError::dominio(
                "COMBO_SIN_COMPONENTES",
                "El combo requiere al menos un componente.",
            ));
        };
        if componentes.is_empty() {
            return Err(ApiError::dominio(
                "COMBO_SIN_COMPONENTES",
                "El combo requiere al menos un componente.",
            ));
        }
        let mut vistos: Vec<String> = Vec::new();
        let mut resultado = Vec::with_capacity(componentes.len());
        for componente in componentes {
            let cantidad = cantidad_componente(componente)?;
            let item = self
                .buscar_item(
                    empresa_id,
                    componente.get("item_id").and_then(Value::as_str),
                    componente.get("sku").and_then(Value::as_str).unwrap_or(""),
                )
                .await?;
            if item.es_combo {
                return Err(ApiError::dominio(
                    "COMPONENTE_INVALIDO",
                    "Un combo no puede incluir otro combo; usa sus productos base.",
                ));
            }
            registrar_componente_unico(&mut vistos, &item.id)?;
            resultado.push(ComponenteResuelto {
                item_id: item.id,
                sku: item.sku,
                nombre: item.nombre,
                cantidad,
            });
        }
        Ok(resultado)
    }

    async fn validar_combo_unico(
        &self,
        empresa_id: &str,
        sku: &str,
        codigo_barras: &str,
        excluir_id: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_combo = match excluir_id {
            Some(_) => {
                "SELECT 1 FROM item.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto'
                    AND upper(codigo) = upper($2) AND eliminado_en IS NULL
                    AND id <> $3::uuid"
            }
            None => {
                "SELECT 1 FROM item.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto'
                    AND upper(codigo) = upper($2) AND eliminado_en IS NULL"
            }
        };
        let mut consulta = sqlx::query(sql_combo).bind(empresa_id).bind(sku);
        if let Some(excluir_id) = excluir_id {
            consulta = consulta.bind(excluir_id);
        }
        let combo = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        if combo.is_some() {
            return Err(ApiError::conflicto(format!(
                "Ya existe un item con el SKU {sku}."
            )));
        }
        if codigo_barras.is_empty() {
            return Ok(());
        }
        let sql_barcode = match excluir_id {
            Some(_) => {
                "SELECT 1 FROM item.items
                  WHERE empresa_id = $1::uuid AND codigo_barras = $2 AND eliminado_en IS NULL
                    AND id <> $3::uuid
                  LIMIT 1"
            }
            None => {
                "SELECT 1 FROM item.items
                  WHERE empresa_id = $1::uuid AND codigo_barras = $2 AND eliminado_en IS NULL
                  LIMIT 1"
            }
        };
        let mut consulta = sqlx::query(sql_barcode)
            .bind(empresa_id)
            .bind(codigo_barras);
        if let Some(excluir_id) = excluir_id {
            consulta = consulta.bind(excluir_id);
        }
        let barcode = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if barcode.is_some() {
            return Err(ApiError::conflicto(format!(
                "El código de barras {codigo_barras} ya está registrado."
            )));
        }
        Ok(())
    }

    async fn resolver_productos(
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

    async fn buscar_item(
        &self,
        empresa_id: &str,
        item_id: Option<&str>,
        sku: &str,
    ) -> Result<ItemResuelto, ApiError> {
        let item_id = item_id.filter(|valor| !valor.is_empty());
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match item_id {
            Some(item_id) => sqlx::query(
                "SELECT id, codigo AS sku, nombre, costo_cents, es_combo
                   FROM item.items
                  WHERE empresa_id = $1::uuid AND tipo = 'producto' AND id = $2::uuid
                    AND eliminado_en IS NULL",
            )
            .bind(empresa_id)
            .bind(item_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => sqlx::query(
                "SELECT id, codigo AS sku, nombre, costo_cents, es_combo
                   FROM item.items
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
        Ok(ItemResuelto {
            id: id.to_string(),
            sku: fila.try_get("sku").map_err(interno)?,
            nombre: fila.try_get("nombre").map_err(interno)?,
            costo_cents: costo_cents.unwrap_or(0) as i64,
            es_combo: fila.try_get("es_combo").map_err(interno)?,
        })
    }
}

// ── funciones puras (probadas sin base de datos) ─────────────────────────
