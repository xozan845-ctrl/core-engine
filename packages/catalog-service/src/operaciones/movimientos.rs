use super::helpers::*;
use super::*;

impl Operaciones {
    pub async fn movimientos(&self, empresa_id: &str, filtros: &Value) -> Result<Value, ApiError> {
        let pagina = filtro_texto(filtros, "pagina");
        let limite = filtro_texto(filtros, "limite");
        let paginacion = parsear_paginacion(pagina.as_deref(), limite.as_deref());
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        let mut condiciones = vec!["empresa_id = $1::uuid".to_string()];
        if let Some(tipo) = filtro_texto(filtros, "tipo").filter(|valor| !valor.is_empty()) {
            params.push(tipo);
            condiciones.push(format!("tipo = ${}", params.len()));
        }
        let where_clause = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total =
            format!("SELECT COUNT(*)::int AS n FROM catalog.movimientos_inventario {where_clause}");
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
            "SELECT {COLUMNAS_MOVIMIENTO}
               FROM catalog.movimientos_inventario {where_clause}
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

        let mut movimientos = Vec::with_capacity(filas.len());
        for fila in &filas {
            movimientos.push(serializar_movimiento(&fila_a_json(fila).map_err(interno)?));
        }
        Ok(crear_pagina(movimientos, total as i64, paginacion))
    }

    pub async fn crear_movimiento(
        &self,
        empresa_id: &str,
        datos: &Value,
        usuario_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        let cabecera = validar_cabecera_movimiento(datos)?;
        let config = self.configuracion(empresa_id).await?;
        let permitir_stock_negativo = config
            .get("permitir_stock_negativo")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let lineas = datos
            .get("lineas")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let productos = self.resolver_productos(empresa_id, &lineas).await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut lineas_salida: Vec<Value> = Vec::with_capacity(lineas.len());
        let mut stock_items: Vec<Value> = Vec::new();
        for (indice, linea) in lineas.iter().enumerate() {
            let producto = &productos[indice];
            let cantidad = entero_de(linea.get("cantidad")).ok_or_else(|| {
                ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad debe ser un entero mayor o igual a 0.",
                )
            })?;
            if cantidad < 0 {
                return Err(ApiError::dominio(
                    "CANTIDAD_INVALIDA",
                    "La cantidad debe ser un entero mayor o igual a 0.",
                ));
            }
            let anterior = producto.stock;
            let nuevo = calcular_stock(&cabecera.tipo, anterior, cantidad);
            if nuevo < 0 && !permitir_stock_negativo {
                return Err(ApiError::dominio(
                    "STOCK_INSUFICIENTE",
                    format!(
                        "Stock insuficiente para {} (disponible {}).",
                        producto.sku, anterior
                    ),
                ));
            }
            if cabecera.tipo != "traslado" {
                sqlx::query(
                    "UPDATE catalog.items
                        SET stock = $3, actualizado_en = NOW()
                      WHERE id = $1::uuid AND empresa_id = $2::uuid AND tipo = 'producto'
                        AND eliminado_en IS NULL",
                )
                .bind(&producto.id)
                .bind(empresa_id)
                .bind(nuevo)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
                let motivo = datos
                    .get("motivo")
                    .filter(|valor| !valor.is_null())
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("movimiento {}", cabecera.tipo));
                sqlx::query(
                    "INSERT INTO catalog.ajustes_stock
                       (id, empresa_id, item_id, cantidad_anterior, cantidad_nueva, motivo,
                        realizado_en)
                     VALUES (gen_random_uuid(), $1::uuid, $2::uuid, $3, $4, $5, NOW())",
                )
                .bind(empresa_id)
                .bind(&producto.id)
                .bind(anterior)
                .bind(nuevo)
                .bind(motivo)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
                stock_items.push(json!({
                    "empresa_id": empresa_id,
                    "sku": producto.sku,
                    "stock_restante": nuevo,
                }));
            }
            lineas_salida.push(json!({
                "item_id": producto.id,
                "sku": producto.sku,
                "nombre": producto.nombre,
                "cantidad": cantidad,
                "stock_anterior": anterior,
                "stock_nuevo": nuevo,
            }));
        }

        let motivo = texto_opcional(datos, "motivo").unwrap_or_default();
        let referencia = texto_opcional(datos, "referencia").unwrap_or_default();
        let lineas_json = Value::Array(lineas_salida).to_string();
        let fila = sqlx::query(&format!(
            "INSERT INTO catalog.movimientos_inventario
               (empresa_id, tipo, motivo, referencia, ubicacion_origen, ubicacion_destino,
                lineas, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7::jsonb, $8)
             RETURNING {COLUMNAS_MOVIMIENTO}"
        ))
        .bind(empresa_id)
        .bind(&cabecera.tipo)
        .bind(&motivo)
        .bind(&referencia)
        .bind(&cabecera.origen)
        .bind(&cabecera.destino)
        .bind(lineas_json)
        .bind(usuario_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let movimiento = fila_a_json(&fila).map_err(interno)?;
        let movimiento_id = movimiento
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        if let Some(outbox) = &self.outbox {
            if !stock_items.is_empty() {
                let tipo_evento = match cabecera.tipo.as_str() {
                    "entrada" => "entrada",
                    "salida" => "salida",
                    _ => "ajuste",
                };
                outbox
                    .insertar_en_transaccion(
                        &mut tx,
                        EVENTO_STOCK_UPDATED,
                        json!({
                            "order_id": format!("movimiento-{movimiento_id}"),
                            "tipo": tipo_evento,
                            "items": stock_items,
                        }),
                    )
                    .await?;
            }
        }
        tx.commit().await.map_err(interno)?;
        Ok(serializar_movimiento(&movimiento))
    }
}
