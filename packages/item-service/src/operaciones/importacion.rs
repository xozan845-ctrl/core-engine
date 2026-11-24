use super::*;

impl Operaciones {
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
