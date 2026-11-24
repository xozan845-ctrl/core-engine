use super::*;

impl Items {
    pub async fn listar(&self, filtros: &Value) -> Result<Value, ApiError> {
        let empresa = la_empresa(filtros.get("empresa_id"))?;
        let paginacion = parsear_paginacion(
            filtros.get("pagina").and_then(Value::as_str),
            filtros.get("limite").and_then(Value::as_str),
        );
        let mut condiciones = vec![
            "i.empresa_id = $1::uuid".to_string(),
            "i.eliminado_en IS NULL".to_string(),
        ];
        let mut params: Vec<String> = vec![empresa.clone()];
        if let Some(tipo) = filtros.get("tipo") {
            let tipo = exigir_tipo(tipo)?;
            params.push(tipo.to_string());
            condiciones.push(format!("i.tipo = ${}", params.len()));
        }
        if let Some(categoria) = texto_opcional(filtros, "categoria").filter(|v| !v.is_empty()) {
            params.push(categoria.to_lowercase());
            condiciones.push(format!("LOWER(i.categoria) = ${}", params.len()));
        }
        if let Some(negocio) = texto_opcional(filtros, "negocio_id").filter(|v| !v.is_empty()) {
            if !es_uuid(negocio) {
                return Err(ApiError::dominio(
                    "NEGOCIO_INVALIDO",
                    "El id de negocio no es un UUID valido.",
                ));
            }
            params.push(negocio.to_string());
            condiciones.push(format!("i.negocio_id = ${}::uuid", params.len()));
        }
        if let Some(q) = texto_opcional(filtros, "q").filter(|v| !v.is_empty()) {
            params.push(format!("%{}%", q.to_lowercase()));
            condiciones.push(format!(
                "(LOWER(i.nombre) LIKE ${0} OR LOWER(i.codigo) LIKE ${0})",
                params.len()
            ));
        }
        let incluir_inactivos = filtros
            .get("incluir_inactivos")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !incluir_inactivos {
            condiciones.push("i.estado = 'activo'".to_string());
        }
        let where_clause = condiciones.join(" AND ");
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total = format!("SELECT COUNT(*)::int AS n FROM item.items i WHERE {where_clause}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");
        let limite = paginacion.limite;
        let offset = (paginacion.pagina - 1) * paginacion.limite;
        let sql = format!(
            "SELECT {COLUMNAS} FROM item.items i WHERE {where_clause}
             ORDER BY creado_en DESC LIMIT ${} OFFSET ${}",
            params.len() + 1,
            params.len() + 2
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta
            .bind(limite)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let mut items = filas
            .iter()
            .map(|fila| {
                let valor = fila_a_json(fila).map_err(interno)?;
                serializar(&valor)
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        self.adjuntar_imagenes(&mut items).await?;
        Ok(crear_pagina(items, total as i64, paginacion))
    }

    pub async fn encontrar_por_id(
        &self,
        id: &str,
        empresa_id: Option<&str>,
    ) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match empresa_id {
            Some(empresa) => {
                sqlx::query(&format!(
                    "SELECT {COLUMNAS} FROM item.items i WHERE i.id = $1::uuid AND i.empresa_id = $2::uuid AND i.eliminado_en IS NULL"
                ))
                .bind(id)
                .bind(empresa)
                .fetch_optional(&mut *tx)
                .await
            }
            None => {
                sqlx::query(&format!(
                    "SELECT {COLUMNAS} FROM item.items i WHERE i.id = $1::uuid AND i.eliminado_en IS NULL"
                ))
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
            }
        }
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(None);
        };
        let valor = fila_a_json(&fila).map_err(interno)?;
        let mut item = serializar(&valor)?;
        self.adjuntar_imagenes(std::slice::from_mut(&mut item))
            .await?;
        Ok(Some(item))
    }

    pub async fn lote_por_skus(&self, skus: &[String]) -> Result<Vec<Value>, ApiError> {
        if skus.is_empty() {
            return Ok(Vec::new());
        }
        let mut unicos: Vec<String> = skus
            .iter()
            .map(|sku| sku.trim().to_lowercase())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        unicos.sort();
        let marcadores = (1..=unicos.len())
            .map(|indice| format!("${indice}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.track_serial,
                    (SELECT COUNT(*)::int FROM item.item_series s
                      WHERE s.item_id = i.id AND s.estado = 'disponible') AS seriales_disponibles,
                    i.estado
               FROM item.items i
              WHERE LOWER(i.codigo) IN ({marcadores})
                AND i.tipo = 'producto' AND i.estado = 'activo' AND i.eliminado_en IS NULL
              ORDER BY i.creado_en DESC"
        );
        let mut consulta = sqlx::query(&sql);
        for sku in &unicos {
            consulta = consulta.bind(sku);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = consulta.fetch_all(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno))
            .collect()
    }

    pub async fn esquemas(&self, empresa_id: Option<&str>) -> Result<Vec<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT id, empresa_id, tipo, categoria, esquema, version, activo
               FROM item.item_atributo_esquemas
              WHERE activo = true AND (empresa_id IS NULL OR empresa_id = $1::uuid)
              ORDER BY empresa_id NULLS FIRST, tipo, categoria, version DESC",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno))
            .collect()
    }

    pub async fn validar_atributos(
        &self,
        empresa_id: &str,
        tipo: &str,
        categoria: &str,
        atributos: Value,
    ) -> Result<Value, ApiError> {
        if !atributos.is_object() {
            return Err(ApiError::dominio(
                "ATRIBUTOS_INVALIDOS",
                "attributes debe ser un objeto JSON.",
            ));
        }
        let categoria = if categoria.trim().is_empty() {
            "general"
        } else {
            categoria.trim()
        };
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT esquema
               FROM item.item_atributo_esquemas
              WHERE activo = true AND tipo = $1
                AND LOWER(categoria) IN (LOWER($2), 'general')
                AND (empresa_id = $3::uuid OR empresa_id IS NULL)
              ORDER BY (empresa_id IS NULL), (LOWER(categoria) = LOWER($2)) DESC, version DESC
              LIMIT 1",
        )
        .bind(tipo)
        .bind(categoria)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Ok(atributos);
        };
        let esquema: Value = fila.try_get("esquema").map_err(interno)?;
        let errores = errores_esquema(&esquema, &atributos);
        if errores.is_empty() {
            Ok(atributos)
        } else {
            Err(ApiError::dominio(
                "ATRIBUTOS_INVALIDOS",
                "Los atributos no cumplen el JSON Schema de la categoría.",
            )
            .con_detalles(json!({ "errores": errores })))
        }
    }

    async fn adjuntar_imagenes(&self, items: &mut [Value]) -> Result<(), ApiError> {
        if items.is_empty() {
            return Ok(());
        }
        let ids = items
            .iter()
            .filter_map(|item| item.get("item_id").and_then(Value::as_str))
            .map(|id| Uuid::parse_str(id).map_err(|_| ApiError::interno()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT id, item_id, objeto_id, orden, es_principal
               FROM item.item_imagenes
              WHERE item_id = ANY($1::uuid[])
              ORDER BY item_id, orden",
        )
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let mut por_item: std::collections::HashMap<String, Vec<Value>> =
            std::collections::HashMap::new();
        for fila in filas {
            let item_id: Uuid = fila.get("item_id");
            let imagen = fila_a_json(&fila).map_err(interno)?;
            por_item
                .entry(item_id.to_string())
                .or_default()
                .push(json!({
                    "id": imagen.get("id"),
                    "objeto_id": imagen.get("objeto_id"),
                    "orden": imagen.get("orden"),
                    "es_principal": imagen.get("es_principal"),
                }));
        }
        for item in items {
            let item_id = item
                .get("item_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let imagenes = por_item.remove(&item_id).unwrap_or_default();
            if let Some(objeto) = item.as_object_mut() {
                objeto.insert("imagenes".into(), Value::Array(imagenes));
            }
        }
        Ok(())
    }
}
