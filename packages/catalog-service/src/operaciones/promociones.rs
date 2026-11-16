use super::helpers::*;
use super::*;

impl Operaciones {
    pub async fn listar_promociones(
        &self,
        empresa_id: &str,
        filtros: &Value,
    ) -> Result<Value, ApiError> {
        let pagina = filtro_texto(filtros, "pagina");
        let limite = filtro_texto(filtros, "limite");
        let paginacion = parsear_paginacion(pagina.as_deref(), limite.as_deref());
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        let mut condiciones = vec![
            "empresa_id = $1::uuid".to_string(),
            "eliminado_en IS NULL".to_string(),
        ];
        if let Some(estado) = filtro_texto(filtros, "estado").filter(|valor| !valor.is_empty()) {
            params.push(estado);
            condiciones.push(format!("estado = ${}", params.len()));
        }
        let where_clause = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total =
            format!("SELECT COUNT(*)::int AS n FROM catalog.promociones {where_clause}");
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
            "SELECT {COLUMNAS_PROMOCION}
               FROM catalog.promociones {where_clause}
              ORDER BY creado_en DESC LIMIT ${} OFFSET ${}",
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

        let mut promociones = Vec::with_capacity(filas.len());
        for fila in &filas {
            promociones.push(serializar_promocion(&fila_a_json(fila).map_err(interno)?));
        }
        Ok(crear_pagina(promociones, total as i64, paginacion))
    }

    pub async fn obtener_promocion(
        &self,
        id: &str,
        empresa_id: &str,
    ) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_PROMOCION}
               FROM catalog.promociones
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL"
        ))
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        fila.map(|fila| fila_a_json(&fila).map(|valor| serializar_promocion(&valor)))
            .transpose()
            .map_err(interno)
    }

    pub async fn crear_promocion(
        &self,
        empresa_id: &str,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        let normalizada = self.normalizar_promocion(empresa_id, datos).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO catalog.promociones
               (empresa_id, nombre, tipo, item_id, sku, cantidad_requerida, cantidad_gratis,
                descuento_porcentaje, fecha_desde, fecha_hasta, estado)
             VALUES ($1::uuid, $2, $3, $4::uuid, $5, $6, $7, $8, $9::date, $10::date, $11)
             {RETURNING_PROMOCION}"
        ))
        .bind(empresa_id)
        .bind(&normalizada.nombre)
        .bind(&normalizada.tipo)
        .bind(normalizada.item_id.as_deref())
        .bind(&normalizada.sku)
        .bind(normalizada.cantidad_requerida)
        .bind(normalizada.cantidad_gratis)
        .bind(normalizada.descuento_porcentaje)
        .bind(&normalizada.fecha_desde)
        .bind(&normalizada.fecha_hasta)
        .bind(&normalizada.estado)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(serializar_promocion(&fila_a_json(&fila).map_err(interno)?)),
            None => Err(ApiError::dominio(
                "PROMOCION_NO_CREADA",
                "No se pudo crear la promoción.",
            )),
        }
    }

    pub async fn actualizar_promocion(
        &self,
        id: &str,
        empresa_id: &str,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self
            .obtener_promocion(id, empresa_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Promoción", id))?;

        let mut combinada = Map::new();
        for clave in ["nombre", "tipo", "sku", "estado"] {
            let valor = datos
                .get(clave)
                .filter(|valor| !valor.is_null())
                .cloned()
                .or_else(|| actual.get(clave).cloned())
                .unwrap_or(Value::Null);
            combinada.insert(clave.to_string(), valor);
        }
        let item_id = datos
            .get("item_id")
            .filter(|valor| !valor.is_null())
            .cloned()
            .or_else(|| {
                actual
                    .get("item_id")
                    .filter(|valor| !valor.is_null())
                    .cloned()
            })
            .unwrap_or(Value::Null);
        combinada.insert("item_id".into(), item_id);
        for clave in [
            "cantidad_requerida",
            "cantidad_gratis",
            "descuento_porcentaje",
            "fecha_desde",
            "fecha_hasta",
        ] {
            let valor = match datos.get(clave) {
                Some(valor) => valor.clone(),
                None => actual.get(clave).cloned().unwrap_or(Value::Null),
            };
            combinada.insert(clave.to_string(), valor);
        }

        let normalizada = self
            .normalizar_promocion(empresa_id, &Value::Object(combinada))
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "UPDATE catalog.promociones SET
               nombre = $3, tipo = $4, item_id = $5::uuid, sku = $6, cantidad_requerida = $7,
               cantidad_gratis = $8, descuento_porcentaje = $9, fecha_desde = $10::date,
               fecha_hasta = $11::date, estado = $12, actualizado_en = NOW()
             WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL
             {RETURNING_PROMOCION}"
        ))
        .bind(id)
        .bind(empresa_id)
        .bind(&normalizada.nombre)
        .bind(&normalizada.tipo)
        .bind(normalizada.item_id.as_deref())
        .bind(&normalizada.sku)
        .bind(normalizada.cantidad_requerida)
        .bind(normalizada.cantidad_gratis)
        .bind(normalizada.descuento_porcentaje)
        .bind(&normalizada.fecha_desde)
        .bind(&normalizada.fecha_hasta)
        .bind(&normalizada.estado)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(serializar_promocion(&fila_a_json(&fila).map_err(interno)?)),
            None => Err(ApiError::no_encontrado("Promoción", id)),
        }
    }

    pub async fn eliminar_promocion(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE catalog.promociones SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Promoción", id));
        }
        Ok(())
    }

    // ── movimientos de inventario ────────────────────────────────────────

    pub(super) async fn normalizar_promocion(
        &self,
        empresa_id: &str,
        datos: &Value,
    ) -> Result<PromocionNormalizada, ApiError> {
        let mut normalizada = validar_promocion(datos)?;
        let item_id = datos
            .get("item_id")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty());
        let sku = datos
            .get("sku")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty());
        if item_id.is_some() || sku.is_some() {
            let item = self
                .buscar_item(empresa_id, item_id, sku.unwrap_or(""))
                .await?;
            normalizada.item_id = Some(item.id);
            normalizada.sku = item.sku;
        }
        Ok(normalizada)
    }
}
