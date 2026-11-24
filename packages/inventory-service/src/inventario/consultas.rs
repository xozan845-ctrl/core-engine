use super::*;

impl Inventario {
    /// Niveles de stock de la bodega desde la proyección CQRS (H-06).
    pub async fn listar(
        &self,
        empresa_id: Option<&str>,
        negocio_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut condiciones = vec![
            "i.tipo = 'producto'".to_string(),
            "i.eliminado_en IS NULL".to_string(),
            "NOT EXISTS (SELECT 1 FROM item.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)"
                .to_string(),
        ];
        let mut params: Vec<String> = Vec::new();
        if let Some(empresa) = empresa_id.filter(|valor| !valor.is_empty()) {
            params.push(empresa.to_string());
            condiciones.insert(0, format!("i.empresa_id = ${}::uuid", params.len()));
        }
        if let Some(negocio) = negocio_id.filter(|valor| !valor.is_empty()) {
            params.push(negocio.to_string());
            condiciones.push(format!("i.negocio_id = ${}::uuid", params.len()));
        }
        let filtro = format!("WHERE {}", condiciones.join(" AND "));
        let sql = format!(
            "WITH reservas AS (
               SELECT COALESCE(item->>'empresa_id', '00000000-0000-0000-0000-000000000000') AS empresa_id,
                      LOWER(item->>'sku') AS sku,
                      SUM((item->>'cantidad')::int) AS reservado
                 FROM inventory.reservas_ordenes ro,
                      jsonb_array_elements(ro.items_json) AS item
                WHERE ro.creado_en > NOW() - INTERVAL '30 minutes'
                GROUP BY 1, 2
             )
             SELECT i.empresa_id, i.codigo AS sku, i.nombre,
                    COALESCE(s.total, 0) + COALESCE(r.reservado, 0) AS stock_fisico,
                    COALESCE(r.reservado, 0)::int AS reservado,
                    COALESCE(s.total, 0)::int AS disponible,
                    CASE WHEN COALESCE(s.total, 0) > 0 THEN 'disponible' ELSE 'agotado' END AS estado
               FROM item.items i
               LEFT JOIN (
                 SELECT item_id, SUM(stock)::int AS total
                   FROM inventory.item_stock_ubicacion
                  GROUP BY item_id
               ) s ON s.item_id = i.id
               LEFT JOIN reservas r
                 ON r.empresa_id = i.empresa_id::text AND r.sku = LOWER(i.codigo)
              {filtro}
              ORDER BY estado DESC, i.nombre ASC"
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta.fetch_all(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let mut lineas = Vec::with_capacity(filas.len());
        for fila in &filas {
            let valor = fila_a_json(fila).map_err(interno)?;
            let mut linea = Map::new();
            linea.insert(
                "empresa_id".into(),
                valor.get("empresa_id").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "sku".into(),
                valor.get("sku").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "nombre".into(),
                valor.get("nombre").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "stock".into(),
                valor.get("stock_fisico").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "disponible".into(),
                valor.get("disponible").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "reservado".into(),
                valor.get("reservado").cloned().unwrap_or(Value::Null),
            );
            linea.insert(
                "estado".into(),
                valor.get("estado").cloned().unwrap_or(Value::Null),
            );
            lineas.push(Value::Object(linea));
        }
        Ok(lineas)
    }

    /// Totales del inventario para las rutas internas.
    /// Registra el stock inicial de un item creado por `item-service`.
    pub async fn stock_inicial(
        &self,
        empresa_id: &str,
        item_id: &str,
        sku: &str,
        cantidad: i64,
        negocio_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        if cantidad < 0 {
            return Err(ApiError::validacion(serde_json::json!([{
                "campo": "cantidad",
                "restricciones": ["min"],
            }])));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let item = sqlx::query(
            "SELECT id, track_inventory FROM item.items
              WHERE id = $1::uuid AND empresa_id = $2::uuid
                AND tipo = 'producto' AND eliminado_en IS NULL",
        )
        .bind(item_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        let Some(item) = item else {
            return Err(ApiError::no_encontrado("Item", item_id));
        };
        if !item.get::<bool, _>("track_inventory") {
            return Err(ApiError::dominio(
                "INVENTARIO_NO_APLICA",
                "El item no controla inventario.",
            ));
        }
        let mut ubicacion: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM inventory.ubicaciones
              WHERE empresa_id = $1::uuid AND activo AND predeterminada
              ORDER BY creado_en, id LIMIT 1",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if ubicacion.is_none() {
            ubicacion = sqlx::query_scalar(
                "SELECT id FROM inventory.ubicaciones
                  WHERE empresa_id = $1::uuid AND activo
                  ORDER BY creado_en, id LIMIT 1",
            )
            .bind(empresa_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
        }
        let ubicacion = match ubicacion {
            Some(id) => id,
            None => sqlx::query_scalar(
                "INSERT INTO inventory.ubicaciones
                   (empresa_id, negocio_id, codigo, nombre, predeterminada)
                 VALUES ($1::uuid, $2::uuid, 'principal', 'Principal', true)
                 RETURNING id",
            )
            .bind(empresa_id)
            .bind(negocio_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?,
        };
        sqlx::query(
            "INSERT INTO inventory.item_stock_ubicacion AS s
               (empresa_id, item_id, ubicacion_id, stock)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4)
             ON CONFLICT (item_id, ubicacion_id) DO UPDATE
               SET stock = EXCLUDED.stock, actualizado_en = NOW()",
        )
        .bind(empresa_id)
        .bind(item_id)
        .bind(ubicacion.to_string())
        .bind(cantidad as i32)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        if let Some(outbox) = &self.outbox {
            outbox
                .insertar_stock_updated(
                    &mut tx,
                    &format!("stock-inicial-{item_id}"),
                    "stock_inicial",
                    vec![json!({
                        "empresa_id": empresa_id,
                        "sku": sku,
                        "stock_restante": cantidad,
                    })],
                )
                .await?;
        }
        tx.commit().await.map_err(interno)?;
        Ok(json!({
            "item_id": item_id,
            "sku": sku,
            "stock": cantidad,
        }))
    }

    /// Stock y series disponibles por lote de SKUs (contrato interno v2).
    /// Si se indica `ubicacion_id`, el stock y los seriales se acotan a esa
    /// ubicación; sin ubicación se conserva el total de la empresa.
    pub async fn stock_lote(
        &self,
        empresa_id: &str,
        skus: &[String],
        ubicacion_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        if skus.is_empty() {
            return Ok(Vec::new());
        }
        if Uuid::parse_str(empresa_id).is_err() {
            return Err(ApiError::validacion(serde_json::json!([{
                "campo": "empresa_id",
                "restricciones": ["uuid"],
            }])));
        }
        let codigos: Vec<String> = skus.iter().map(|sku| sku.to_uppercase()).collect();
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT i.id AS item_id, i.empresa_id, i.codigo AS sku,
                    i.track_serial, i.estado,
                    COALESCE((
                      SELECT SUM(s.stock)::int
                        FROM inventory.item_stock_ubicacion s
                       WHERE s.item_id = i.id
                         AND ($3::uuid IS NULL OR s.ubicacion_id = $3::uuid)
                    ), 0) AS stock,
                    COALESCE((
                      SELECT COUNT(*)::int
                        FROM item.item_series se
                       WHERE se.item_id = i.id AND se.estado = 'disponible'
                         AND ($3::uuid IS NULL OR se.ubicacion_id = $3::uuid)
                    ), 0) AS seriales_disponibles
               FROM item.items i
              WHERE i.empresa_id = $1::uuid
                AND i.tipo = 'producto'
                AND i.eliminado_en IS NULL
                AND upper(i.codigo) = ANY($2::text[])
              ORDER BY i.codigo",
        )
        .bind(empresa_id)
        .bind(&codigos)
        .bind(ubicacion_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(fila_a_json)
            .collect::<Result<Vec<_>, _>>()
            .map_err(interno)
    }

    pub async fn resumen(
        &self,
        empresa_id: Option<&str>,
        negocio_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        let mut condiciones = vec![
            "i.tipo = 'producto'".to_string(),
            "i.eliminado_en IS NULL".to_string(),
            "NOT EXISTS (SELECT 1 FROM item.items v WHERE v.parent_item_id = i.id AND v.eliminado_en IS NULL)"
                .to_string(),
        ];
        let mut params: Vec<String> = Vec::new();
        if let Some(empresa) = empresa_id.filter(|valor| !valor.is_empty()) {
            params.push(empresa.to_string());
            condiciones.insert(0, format!("i.empresa_id = ${}::uuid", params.len()));
        }
        if let Some(negocio) = negocio_id.filter(|valor| !valor.is_empty()) {
            params.push(negocio.to_string());
            condiciones.push(format!("i.negocio_id = ${}::uuid", params.len()));
        }
        let filtro = format!("WHERE {}", condiciones.join(" AND "));
        let sql = format!(
            "WITH items AS (
               SELECT i.id,
                      COALESCE((
                        SELECT SUM(s.stock)::int
                          FROM inventory.item_stock_ubicacion s
                         WHERE s.item_id = i.id
                      ), 0) AS stock
                 FROM item.items i {filtro}
             )
             SELECT COUNT(*)::int AS productos,
                    COUNT(*) FILTER (WHERE stock = 0)::int AS agotados,
                    COALESCE(SUM(stock), 0)::int AS stock_total
               FROM items"
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Ok(json!({
                "productos": 0,
                "agotados": 0,
                "stock_total": 0,
            })),
        }
    }

    pub async fn listar_lotes(
        &self,
        empresa_id: &str,
        item_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut condiciones = vec!["empresa_id = $1::uuid".to_string()];
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        if let Some(item) = item_id.filter(|valor| !valor.is_empty()) {
            params.push(item.to_string());
            condiciones.push("item_id = $2::uuid".to_string());
        }
        let sql = format!(
            "SELECT id, empresa_id, item_id, proveedor_id, codigo_lote, vence_en,
                    cantidad_inicial, cantidad_disponible
               FROM inventory.lotes_inventario
              WHERE {}
              ORDER BY vence_en NULLS LAST, codigo_lote",
            condiciones.join(" AND ")
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta.fetch_all(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas.iter().map(lote_a_json).collect()
    }
}
