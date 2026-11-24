use super::*;

impl Inventario {
    /// Configuración de inventario de la empresa o los valores por defecto.
    pub async fn configuracion(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT empresa_id, negocio_id, metodo_costeo, permitir_stock_negativo,
                    alerta_stock_bajo, actualizar_costo_en_compra, unidad_defecto,
                    actualizado_en
               FROM inventory.configuracion_inventario
              WHERE empresa_id = $1::uuid
                AND (negocio_id = $2::uuid OR negocio_id IS NULL)
              ORDER BY (negocio_id IS NOT NULL) DESC
              LIMIT 1",
        )
        .bind(empresa_id)
        .bind(negocio_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Ok(json!({
                "empresa_id": empresa_id,
                "negocio_id": negocio_id,
                "metodo_costeo": "promedio",
                "permitir_stock_negativo": false,
                "alerta_stock_bajo": true,
                "actualizar_costo_en_compra": true,
                "unidad_defecto": "Unidad (u)",
                "actualizado_en": "1970-01-01T00:00:00.000Z",
            })),
        }
    }

    /// Upsert de la configuración: cada campo ausente conserva el valor actual.
    pub async fn guardar_configuracion(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
        cambios: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.configuracion(empresa_id, negocio_id).await?;
        let actual_metodo = actual
            .get("metodo_costeo")
            .and_then(Value::as_str)
            .unwrap_or("promedio")
            .to_string();
        let metodo = match cambios.get("metodo_costeo") {
            None | Some(Value::Null) => actual_metodo,
            Some(Value::String(valor)) => valor.clone(),
            Some(_) => String::new(),
        };
        if !METODOS_COSTEO.contains(&metodo.as_str()) {
            return Err(ApiError::dominio(
                "METODO_COSTEO_INVALIDO",
                "El método de costeo no es válido.",
            ));
        }
        let permitir_stock_negativo = booleano_configuracion(
            cambios,
            "permitir_stock_negativo",
            actual
                .get("permitir_stock_negativo")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        );
        let alerta_stock_bajo = booleano_configuracion(
            cambios,
            "alerta_stock_bajo",
            actual
                .get("alerta_stock_bajo")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        );
        let actualizar_costo_en_compra = booleano_configuracion(
            cambios,
            "actualizar_costo_en_compra",
            actual
                .get("actualizar_costo_en_compra")
                .and_then(Value::as_bool)
                .unwrap_or(true),
        );
        let unidad_actual = actual
            .get("unidad_defecto")
            .and_then(Value::as_str)
            .unwrap_or("Unidad (u)")
            .trim()
            .to_string();
        let unidad = match cambios.get("unidad_defecto") {
            None | Some(Value::Null) => unidad_actual,
            Some(Value::String(valor)) => valor.trim().to_string(),
            Some(_) => unidad_actual,
        };
        let unidad_defecto = if unidad.is_empty() {
            "Unidad (u)".to_string()
        } else {
            unidad
        };

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO inventory.configuracion_inventario
               (empresa_id, negocio_id, metodo_costeo, permitir_stock_negativo, alerta_stock_bajo,
                actualizar_costo_en_compra, unidad_defecto)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7)
             ON CONFLICT (empresa_id, COALESCE(negocio_id, '00000000-0000-0000-0000-000000000000'::uuid))
             DO UPDATE SET
               metodo_costeo = EXCLUDED.metodo_costeo,
               permitir_stock_negativo = EXCLUDED.permitir_stock_negativo,
               alerta_stock_bajo = EXCLUDED.alerta_stock_bajo,
               actualizar_costo_en_compra = EXCLUDED.actualizar_costo_en_compra,
               unidad_defecto = EXCLUDED.unidad_defecto,
               actualizado_en = NOW()
             RETURNING empresa_id, negocio_id, metodo_costeo, permitir_stock_negativo,
                       alerta_stock_bajo, actualizar_costo_en_compra, unidad_defecto,
                       actualizado_en",
        )
        .bind(empresa_id)
        .bind(negocio_id)
        .bind(&metodo)
        .bind(permitir_stock_negativo)
        .bind(alerta_stock_bajo)
        .bind(actualizar_costo_en_compra)
        .bind(&unidad_defecto)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_a_json(&fila).map_err(interno),
            None => Err(ApiError::dominio(
                "CONFIGURACION_NO_GUARDADA",
                "No se pudo guardar la configuración.",
            )),
        }
    }
}
