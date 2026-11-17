use super::helpers::*;
use super::*;

impl Productos {
    pub async fn crear_variante(
        &self,
        padre_id: &str,
        datos: &Value,
        empresa_id: &str,
    ) -> Result<Value, ApiError> {
        let padre = self
            .encontrar_por_id(padre_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", padre_id))?;
        if padre
            .get("producto_padre_id")
            .and_then(Value::as_str)
            .is_some()
        {
            return Err(ApiError::dominio(
                "NO_ES_PADRE",
                "Solo un producto padre puede recibir variantes.",
            ));
        }
        let variantes_actuales = padre
            .get("variantes")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        if variantes_actuales >= MAX_VARIANTES {
            return Err(ApiError::dominio(
                "DEMASIADAS_VARIANTES",
                format!("Un producto admite como máximo {MAX_VARIANTES} variantes."),
            ));
        }
        self.validar_sin_ofertas_activas(padre_id).await?;
        let categoria = texto(&padre, "categoria");
        let atributos = atributos_de(datos);
        if let Some(items) = &self.items {
            items
                .validar_atributos(empresa_id, "producto", &categoria, atributos)
                .await?;
        }
        let sku = normalizar_sku(&texto(datos, "sku"))?;
        let stock = validar_stock(datos.get("stock"))?;
        let precio = validar_precio(datos.get("precio_base"))?;
        if self.sku_existe(empresa_id, &sku).await? {
            return Err(ApiError::conflicto(format!(
                "Ya existe un producto con el SKU {sku} en esta empresa."
            )));
        }
        let imagenes = imagenes_de(datos.get("imagenes"));
        self.validar_imagenes(empresa_id, &imagenes).await?;

        let variante_id = Uuid::new_v4();
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "INSERT INTO item.items
               (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
                track_inventory, parent_item_id, atributos)
             VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, $6, true, $7::uuid, $8::jsonb)",
        )
        .bind(variante_id)
        .bind(empresa_id)
        .bind(&sku)
        .bind(
            datos
                .get("nombre")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    padre
                        .get("nombre")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default()
                .trim()
                .to_string(),
        )
        .bind(texto(&padre, "descripcion"))
        .bind(&categoria)
        .bind(padre_id)
        .bind(atributos_de(datos).to_string())
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        insertar_imagenes(&mut tx, empresa_id, variante_id, &imagenes).await?;
        recalcular_padre(&mut tx, Uuid::parse_str(padre_id).map_err(interno)?).await?;
        tx.commit().await.map_err(interno)?;

        if let Some(proyecciones) = self.items.as_ref().and_then(|items| items.proyecciones()) {
            let resultado = async {
                proyecciones
                    .fijar_precio(empresa_id, &variante_id.to_string(), precio.centavos)
                    .await?;
                proyecciones
                    .stock_inicial(empresa_id, &variante_id.to_string(), &sku, stock, None)
                    .await?;
                Ok::<(), ApiError>(())
            }
            .await;
            if let Err(error) = resultado {
                let mut tx = self.db.transaccion().await.map_err(interno)?;
                sqlx::query("DELETE FROM item.items WHERE id = $1::uuid")
                    .bind(variante_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(interno)?;
                tx.commit().await.map_err(interno)?;
                return Err(error);
            }
        }

        self.encontrar_por_id(&variante_id.to_string(), Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Variante", &variante_id.to_string()))
    }

    /// H-03: edición del catálogo. Los cambios de precio base van al histórico
    /// (RN-08). El stock se ajusta en `inventory-service`; aquí se rechaza.
    pub async fn actualizar_variante(
        &self,
        padre_id: &str,
        variante_id: &str,
        cambios: &Value,
        empresa_id: &str,
    ) -> Result<Value, ApiError> {
        let padre = self
            .encontrar_por_id(padre_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", padre_id))?;
        let pertenece = padre
            .get("variantes")
            .and_then(Value::as_array)
            .map(|variantes| {
                variantes
                    .iter()
                    .any(|variante| variante.get("id").and_then(Value::as_str) == Some(variante_id))
            })
            .unwrap_or(false);
        if !pertenece {
            return Err(ApiError::no_encontrado("Variante", variante_id));
        }
        self.actualizar(variante_id, cambios, Some(empresa_id))
            .await
    }

    /// Baja lógica del producto y de sus variantes (R-DB-5).
    pub async fn eliminar_variante(
        &self,
        padre_id: &str,
        variante_id: &str,
        empresa_id: &str,
    ) -> Result<(), ApiError> {
        let padre = self
            .encontrar_por_id(padre_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", padre_id))?;
        let pertenece = padre
            .get("variantes")
            .and_then(Value::as_array)
            .map(|variantes| {
                variantes
                    .iter()
                    .any(|variante| variante.get("id").and_then(Value::as_str) == Some(variante_id))
            })
            .unwrap_or(false);
        if !pertenece {
            return Err(ApiError::no_encontrado("Variante", variante_id));
        }
        self.validar_sin_ofertas_activas(variante_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "UPDATE item.items
                SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND parent_item_id = $3::uuid
                AND eliminado_en IS NULL",
        )
        .bind(variante_id)
        .bind(empresa_id)
        .bind(padre_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        recalcular_padre(&mut tx, Uuid::parse_str(padre_id).map_err(interno)?).await?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }
}
