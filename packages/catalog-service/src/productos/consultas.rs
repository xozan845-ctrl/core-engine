use super::helpers::*;
use super::*;

impl Productos {
    pub async fn listar(&self, filtros: &Value) -> Result<Value, ApiError> {
        let empresa_id = exigir_empresa(filtros.get("empresa_id"))?;
        let paginacion = paginacion_de(filtros);
        let mut params: Vec<String> = vec![empresa_id];
        let mut condiciones: Vec<String> = vec![
            "i.empresa_id = $1::uuid".to_string(),
            "i.tipo = 'producto'".to_string(),
            "i.parent_item_id IS NULL".to_string(),
            "i.eliminado_en IS NULL".to_string(),
        ];
        if !filtros
            .get("incluir_inactivos")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            condiciones.push("i.estado = 'activo'".to_string());
        }
        if let Some(q) = filtros.get("q").filter(|valor| valor_verdadero(valor)) {
            params.push(format!("%{}%", texto_js(q).to_lowercase()));
            condiciones.push(format!(
                "(LOWER(i.nombre) LIKE ${0} OR LOWER(i.codigo) LIKE ${0})",
                params.len()
            ));
        }
        if let Some(estado) = filtros.get("estado").filter(|valor| valor_verdadero(valor)) {
            condiciones.push(if texto_js(estado) == "disponible" {
                "i.stock > 0".to_string()
            } else {
                "i.stock = 0".to_string()
            });
        }
        if let Some(categoria) = filtros
            .get("categoria")
            .filter(|valor| valor_verdadero(valor))
        {
            params.push(texto_js(categoria));
            condiciones.push(format!("LOWER(i.categoria) = LOWER(${})", params.len()));
        }
        let where_sql = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total = format!("SELECT COUNT(*)::int AS n FROM catalog.items i {where_sql}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");
        let offset = (paginacion.pagina - 1) * paginacion.limite;
        let sql = format!(
            "SELECT {COLUMNAS_PRODUCTO},
                    {EXISTE_VARIANTES}
               FROM catalog.items i {where_sql}
              ORDER BY i.creado_en DESC LIMIT ${} OFFSET ${}",
            params.len() + 1,
            params.len() + 2
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta
            .bind(paginacion.limite)
            .bind(offset)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let ids = filas
            .iter()
            .map(|fila| fila.get::<Uuid, _>("id").to_string())
            .collect::<Vec<_>>();
        let imagenes = self.imagenes_de(&ids).await?;
        let items = filas
            .iter()
            .map(|fila| {
                let valor = fila_a_json(fila).map_err(interno)?;
                let id = texto(&valor, "id");
                let propias = imagenes.get(&id).map(Vec::as_slice).unwrap_or(&[]);
                serializar(&valor, propias, Vec::new())
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        Ok(crear_pagina(items, total as i64, paginacion))
    }

    pub async fn encontrar_por_id(
        &self,
        id: &str,
        empresa_id: Option<&str>,
        incluir_inactivos: bool,
    ) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut sql = format!(
            "SELECT {COLUMNAS_PRODUCTO}, {EXISTE_VARIANTES}
               FROM catalog.items i
              WHERE i.id = $1::uuid AND i.tipo = 'producto' AND i.eliminado_en IS NULL"
        );
        if empresa_id.is_some() {
            sql.push_str(" AND i.empresa_id = $2::uuid");
        }
        if !incluir_inactivos {
            sql.push_str(" AND i.estado = 'activo'");
        }
        let mut consulta = sqlx::query(&sql).bind(id);
        if let Some(empresa) = empresa_id {
            consulta = consulta.bind(empresa);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => {
                let valor = fila_a_json(&fila).map_err(interno)?;
                self.detallar(&valor).await.map(Some)
            }
            None => Ok(None),
        }
    }

    pub async fn encontrar_por_sku(
        &self,
        sku: &str,
        empresa_id: Option<&str>,
        incluir_inactivos: bool,
    ) -> Result<Option<Value>, ApiError> {
        let mut sql = format!(
            "SELECT {COLUMNAS_PRODUCTO}, {EXISTE_VARIANTES}
               FROM catalog.items i
              WHERE LOWER(i.codigo) = LOWER($1) AND i.tipo = 'producto'
                AND i.eliminado_en IS NULL"
        );
        if empresa_id.is_some() {
            sql.push_str(" AND i.empresa_id = $2::uuid");
        }
        if !incluir_inactivos {
            sql.push_str(" AND i.estado = 'activo'");
        }
        let mut consulta = sqlx::query(&sql).bind(sku);
        if let Some(empresa) = empresa_id {
            consulta = consulta.bind(empresa);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => {
                let valor = fila_a_json(&fila).map_err(interno)?;
                self.detallar(&valor).await.map(Some)
            }
            None => Ok(None),
        }
    }

    pub async fn encontrar_por_codigo_barras(
        &self,
        codigo: &str,
        empresa_id: &str,
        incluir_inactivos: bool,
    ) -> Result<Option<Value>, ApiError> {
        let codigo_barras = codigo.trim();
        if codigo_barras.is_empty() {
            return Ok(None);
        }
        let filtro_estado = if incluir_inactivos {
            ""
        } else {
            "AND i.estado = 'activo'"
        };
        let sql = format!(
            "SELECT {COLUMNAS_PRODUCTO}, {EXISTE_VARIANTES}
               FROM catalog.items i
              WHERE i.empresa_id = $1::uuid AND i.codigo_barras = $2
                AND i.tipo = 'producto' AND i.eliminado_en IS NULL
                {filtro_estado}
              LIMIT 1"
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&sql)
            .bind(empresa_id)
            .bind(codigo_barras)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => {
                let valor = fila_a_json(&fila).map_err(interno)?;
                self.detallar(&valor).await.map(Some)
            }
            None => Ok(None),
        }
    }

    /// Consulta por lote de códigos. Un código puede repetirse en empresas
    /// distintas, por eso la fila incluye `empresa_id` (ADR-27).
    pub async fn lote_por_skus(&self, skus: &[String]) -> Result<Value, ApiError> {
        if skus.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }
        let mut unicos: Vec<String> = Vec::new();
        let mut vistos: HashSet<String> = HashSet::new();
        for sku in skus {
            let normalizado = sku.to_lowercase();
            if vistos.insert(normalizado.clone()) {
                unicos.push(normalizado);
            }
        }
        let marcadores = (1..=unicos.len())
            .map(|indice| format!("${indice}"))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.stock,
                    CASE WHEN i.stock > 0 THEN 'disponible' ELSE 'agotado' END AS estado
               FROM catalog.items i
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
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }

    /// Stock restante por producto (alimenta `stock.updated` para RN-02).
    pub async fn stock_actual_de(&self, items: &Value) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let salida = self.stock_actual_de_en_transaccion(&mut tx, items).await?;
        tx.commit().await.map_err(interno)?;
        Ok(salida)
    }

    pub async fn stock_actual_de_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let claves = claves_stock(items);
        if claves.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }
        let filas = sqlx::query(
            "SELECT i.empresa_id, LOWER(i.codigo) AS sku, i.stock AS stock_restante
               FROM catalog.items i
               JOIN jsonb_to_recordset($1::jsonb) AS x(empresa_id uuid, sku text)
                 ON i.empresa_id = x.empresa_id AND LOWER(i.codigo) = LOWER(x.sku)
              WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL",
        )
        .bind(Value::Array(claves).to_string())
        .fetch_all(&mut **tx)
        .await
        .map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }

    pub async fn resumen_inventario(&self, empresa_id: Option<&str>) -> Result<Value, ApiError> {
        let (filtro, params): (&str, Vec<&str>) = match empresa_id {
            Some(empresa) => (
                "WHERE i.empresa_id = $1::uuid AND i.tipo = 'producto' AND i.eliminado_en IS NULL
                   AND NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)",
                vec![empresa],
            ),
            None => (
                "WHERE i.tipo = 'producto' AND i.eliminado_en IS NULL
                   AND NOT EXISTS (SELECT 1 FROM catalog.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)",
                Vec::new(),
            ),
        };
        let sql = format!(
            "SELECT COUNT(*)::int AS productos,
                    COUNT(*) FILTER (WHERE i.stock = 0)::int AS agotados,
                    COALESCE(SUM(i.stock), 0)::int AS stock_total,
                    COALESCE(SUM(i.stock * i.precio_cents), 0)::int AS valor_inventario_cents
               FROM catalog.items i {filtro}"
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in params {
            consulta = consulta.bind(parametro);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Ok(json!({
                "productos": 0,
                "agotados": 0,
                "stock_total": 0,
                "valor_inventario_cents": 0,
            })),
        }
    }

    // ── helpers de persistencia ───────────────────────────────────────────

    pub(super) async fn detallar(&self, fila: &Value) -> Result<Value, ApiError> {
        let hijos = if fila
            .get("producto_padre_id")
            .map(Value::is_null)
            .unwrap_or(true)
        {
            let sql = format!(
                "SELECT {COLUMNAS_PRODUCTO}, false AS tiene_variantes
                   FROM catalog.items i
                  WHERE i.parent_item_id = $1::uuid AND i.tipo = 'producto'
                    AND i.eliminado_en IS NULL
                  ORDER BY i.creado_en ASC"
            );
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let filas = sqlx::query(&sql)
                .bind(texto(fila, "id"))
                .fetch_all(&mut *tx)
                .await
                .map_err(interno)?;
            tx.commit().await.map_err(interno)?;
            filas
        } else {
            Vec::new()
        };
        let mut ids = vec![texto(fila, "id")];
        let valores_hijos = hijos
            .iter()
            .map(|hijo| fila_a_json(hijo).map_err(interno))
            .collect::<Result<Vec<_>, ApiError>>()?;
        ids.extend(valores_hijos.iter().map(|hijo| texto(hijo, "id")));
        let imagenes = self.imagenes_de(&ids).await?;
        let variantes = valores_hijos
            .iter()
            .map(|hijo| {
                let id = texto(hijo, "id");
                let propias = imagenes.get(&id).map(Vec::as_slice).unwrap_or(&[]);
                serializar(hijo, propias, Vec::new())
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        let id = texto(fila, "id");
        let propias = imagenes.get(&id).map(Vec::as_slice).unwrap_or(&[]);
        serializar(fila, propias, variantes)
    }

    pub(super) async fn imagenes_de(
        &self,
        ids: &[String],
    ) -> Result<HashMap<String, Vec<Value>>, ApiError> {
        let mut mapa: HashMap<String, Vec<Value>> = HashMap::new();
        if ids.is_empty() {
            return Ok(mapa);
        }
        let ids = ids
            .iter()
            .map(|id| Uuid::parse_str(id).map_err(interno))
            .collect::<Result<Vec<_>, _>>()?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT id, item_id, objeto_id, orden, es_principal
               FROM catalog.item_imagenes
              WHERE item_id = ANY($1::uuid[])
              ORDER BY item_id, orden",
        )
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        for fila in filas {
            let valor = fila_a_json(&fila).map_err(interno)?;
            let item_id = texto(&valor, "item_id");
            mapa.entry(item_id)
                .or_default()
                .push(serializar_imagen(&valor));
        }
        Ok(mapa)
    }
}
