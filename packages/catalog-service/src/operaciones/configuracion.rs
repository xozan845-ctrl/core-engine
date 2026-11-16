use super::helpers::*;
use super::*;

impl Operaciones {
    // ── configuración ────────────────────────────────────────────────────

    pub async fn configuracion(&self, empresa_id: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_CONFIGURACION}
               FROM catalog.configuracion_inventario WHERE empresa_id = $1::uuid"
        ))
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => serializar_configuracion(&fila_a_json(&fila).map_err(interno)?),
            None => Ok(configuracion_por_defecto(empresa_id)),
        }
    }

    pub async fn guardar_configuracion(
        &self,
        empresa_id: &str,
        cambios: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.configuracion(empresa_id).await?;
        let metodo = cambios
            .get("metodo_costeo")
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                actual
                    .get("metodo_costeo")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            });
        if !METODOS_COSTEO.contains(&metodo) {
            return Err(ApiError::dominio(
                "METODO_COSTEO_INVALIDO",
                "El método de costeo no es válido.",
            ));
        }
        let booleano = |clave: &str| {
            cambios
                .get(clave)
                .and_then(Value::as_bool)
                .unwrap_or_else(|| actual.get(clave).and_then(Value::as_bool).unwrap_or(false))
        };
        let unidad = cambios
            .get("unidad_defecto")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_else(|| {
                actual
                    .get("unidad_defecto")
                    .and_then(Value::as_str)
                    .unwrap_or("")
            })
            .to_string();
        let unidad = if unidad.is_empty() {
            UNIDAD_DEFECTO.to_string()
        } else {
            unidad
        };

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO catalog.configuracion_inventario
               (empresa_id, metodo_costeo, permitir_stock_negativo, alerta_stock_bajo,
                actualizar_costo_en_compra, unidad_defecto)
             VALUES ($1::uuid, $2, $3, $4, $5, $6)
             ON CONFLICT (empresa_id) DO UPDATE SET
               metodo_costeo = EXCLUDED.metodo_costeo,
               permitir_stock_negativo = EXCLUDED.permitir_stock_negativo,
               alerta_stock_bajo = EXCLUDED.alerta_stock_bajo,
               actualizar_costo_en_compra = EXCLUDED.actualizar_costo_en_compra,
               unidad_defecto = EXCLUDED.unidad_defecto,
               actualizado_en = NOW()
             RETURNING {COLUMNAS_CONFIGURACION}"
        ))
        .bind(empresa_id)
        .bind(metodo)
        .bind(booleano("permitir_stock_negativo"))
        .bind(booleano("alerta_stock_bajo"))
        .bind(booleano("actualizar_costo_en_compra"))
        .bind(&unidad)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => serializar_configuracion(&fila_a_json(&fila).map_err(interno)?),
            None => Err(ApiError::dominio(
                "CONFIGURACION_NO_GUARDADA",
                "No se pudo guardar la configuración.",
            )),
        }
    }

    // ── importación ──────────────────────────────────────────────────────

    pub async fn importar(&self, empresa_id: &str, filas: &[Value]) -> Result<Value, ApiError> {
        if filas.is_empty() {
            return Err(ApiError::dominio(
                "IMPORTACION_VACIA",
                "La importación no contiene filas.",
            ));
        }
        if filas.len() > MAX_IMPORTACION {
            return Err(ApiError::dominio(
                "IMPORTACION_DEMASIADO_GRANDE",
                format!("El máximo es {MAX_IMPORTACION} filas por importación."),
            ));
        }
        self.productos.validar_empresa_catalogo(empresa_id).await?;
        let mut importados = 0usize;
        let mut omitidos = 0usize;
        let mut errores: Vec<String> = Vec::new();
        let opciones = json!({ "empresa_validada": true });
        for (indice, fila) in filas.iter().enumerate() {
            let datos = datos_producto_importado(fila, empresa_id);
            match self.productos.crear(&datos, Some(&opciones)).await {
                Ok(_) => importados += 1,
                Err(error) if error.codigo == "CONFLICTO" => omitidos += 1,
                Err(error) => {
                    errores.push(format!("Fila {}: {}", indice + 1, error.mensaje));
                }
            }
        }
        Ok(json!({
            "importados": importados,
            "omitidos": omitidos,
            "errores": errores,
        }))
    }

    // ── escaneo ──────────────────────────────────────────────────────────

    pub async fn escanear(&self, empresa_id: &str, codigo: &str) -> Result<Value, ApiError> {
        let valor = codigo.trim();
        if valor.is_empty() {
            return Err(ApiError::dominio(
                "CODIGO_REQUERIDO",
                "Indica un código para escanear.",
            ));
        }
        if let Some(producto) = self
            .productos
            .encontrar_por_codigo_barras(valor, empresa_id, false)
            .await?
        {
            return Ok(json!({
                "tipo": "producto",
                "producto": producto,
                "servicio": Value::Null,
            }));
        }
        if let Some(servicio) = self
            .servicios
            .encontrar_por_codigo(valor, empresa_id)
            .await?
        {
            return Ok(json!({
                "tipo": "servicio",
                "producto": Value::Null,
                "servicio": servicio,
            }));
        }
        Err(ApiError::no_encontrado("Código", valor))
    }

    // ── combos ───────────────────────────────────────────────────────────
}
