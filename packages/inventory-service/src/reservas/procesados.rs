use super::*;

impl Reservas {
    pub async fn stock_actual_de_en_transaccion(
        tx: &mut Transaction<'_, Postgres>,
        items: &Value,
    ) -> Result<Value, ApiError> {
        let claves = claves_stock(items);
        if claves.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }
        let filas = sqlx::query(
            "SELECT x.empresa_id, x.sku,
                    COALESCE((
                      SELECT SUM(s.stock)::int
                        FROM inventory.item_stock_ubicacion s
                       WHERE s.item_id = i.id
                         AND (x.ubicacion_id IS NULL OR s.ubicacion_id = x.ubicacion_id)
                    ), 0) AS stock_restante
               FROM item.items i
               JOIN jsonb_to_recordset($1::jsonb) AS x(empresa_id uuid, sku text, ubicacion_id uuid)
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

    pub async fn esta_procesado_en_transaccion(
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
    ) -> Result<bool, ApiError> {
        let fila =
            sqlx::query("SELECT 1 FROM inventory.eventos_procesados WHERE event_id = $1::uuid")
                .bind(event_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(interno)?;
        Ok(fila.is_some())
    }

    pub async fn registrar_procesado_en_transaccion(
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
        tipo: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO inventory.eventos_procesados (event_id, tipo, procesado_en)
             VALUES ($1::uuid, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(tipo)
        .execute(&mut **tx)
        .await
        .map_err(interno)?;
        Ok(())
    }
}
