use super::helpers::*;
use super::*;

impl Productos {
    /// Agrega una imagen (máx. 3) a un producto o variante.
    pub async fn agregar_imagen(
        &self,
        producto_id: &str,
        imagen: &Value,
        empresa_id: &str,
    ) -> Result<Value, ApiError> {
        self.encontrar_por_id(producto_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", producto_id))?;
        let imagen = imagen_de_valor(imagen);
        self.validar_imagenes(empresa_id, std::slice::from_ref(&imagen))
            .await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let actuales = sqlx::query(
            "SELECT id, item_id, objeto_id, orden, es_principal
               FROM item.item_imagenes
              WHERE item_id = $1::uuid
              ORDER BY orden
              FOR UPDATE",
        )
        .bind(producto_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        if actuales.len() as i64 >= MAX_IMAGENES {
            return Err(ApiError::dominio(
                "LIMITE_IMAGENES",
                format!("Cada producto o variante admite hasta {MAX_IMAGENES} imágenes."),
            ));
        }
        let usados: HashSet<i64> = actuales
            .iter()
            .map(|fila| fila.get::<i32, _>("orden") as i64)
            .collect();
        let orden = imagen.orden.unwrap_or_else(|| siguiente_orden(&usados));
        if !(0..MAX_IMAGENES).contains(&orden) || usados.contains(&orden) {
            return Err(ApiError::dominio(
                "IMAGEN_ORDEN_INVALIDO",
                "El orden de la imagen no es válido.",
            ));
        }
        let principal = imagen.es_principal;
        if principal {
            sqlx::query(
                "UPDATE item.item_imagenes SET es_principal = false WHERE item_id = $1::uuid",
            )
            .bind(producto_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        }
        sqlx::query(
            "INSERT INTO item.item_imagenes
               (id, empresa_id, item_id, objeto_id, orden, es_principal)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6)",
        )
        .bind(Uuid::new_v4())
        .bind(empresa_id)
        .bind(producto_id)
        .bind(&imagen.objeto_id)
        .bind(orden)
        .bind(principal)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        let habia_principal = actuales
            .iter()
            .any(|fila| fila.get::<bool, _>("es_principal"));
        if !principal && !habia_principal {
            promover_primera_imagen(&mut tx, producto_id).await?;
        }
        tx.commit().await.map_err(interno)?;

        self.encontrar_por_id(producto_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", producto_id))
    }

    pub async fn eliminar_imagen(
        &self,
        producto_id: &str,
        imagen_id: &str,
        empresa_id: &str,
    ) -> Result<(), ApiError> {
        let producto = self
            .encontrar_por_id(producto_id, Some(empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", producto_id))?;
        let imagen = producto
            .get("imagenes")
            .and_then(Value::as_array)
            .and_then(|imagenes| {
                imagenes
                    .iter()
                    .find(|imagen| imagen.get("id").and_then(Value::as_str) == Some(imagen_id))
            });
        let Some(imagen) = imagen else {
            return Err(ApiError::no_encontrado("Imagen", imagen_id));
        };
        // La oferta congela el objeto de la imagen principal: si está viva,
        // borrarla dejaría una URL rota en la tienda pública.
        if imagen
            .get("es_principal")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            self.validar_sin_ofertas_activas(producto_id).await?;
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let eliminada = sqlx::query(
            "DELETE FROM item.item_imagenes
              WHERE id = $1::uuid AND item_id = $2::uuid AND empresa_id = $3::uuid
              RETURNING es_principal",
        )
        .bind(imagen_id)
        .bind(producto_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if eliminada
            .as_ref()
            .map(|fila| fila.get::<bool, _>("es_principal"))
            .unwrap_or(false)
        {
            promover_primera_imagen(&mut tx, producto_id).await?;
        }
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    // ── helpers de persistencia ───────────────────────────────────────────

    pub(super) async fn validar_imagenes(
        &self,
        empresa_id: &str,
        imagenes: &[ImagenInput],
    ) -> Result<(), ApiError> {
        if imagenes.len() as i64 > MAX_IMAGENES {
            return Err(ApiError::dominio(
                "LIMITE_IMAGENES",
                format!("Cada producto o variante admite hasta {MAX_IMAGENES} imágenes."),
            ));
        }
        let mut ordenes: HashSet<i64> = HashSet::new();
        let mut principales = 0usize;
        for imagen in imagenes {
            if !es_uuid(&imagen.objeto_id) {
                return Err(ApiError::dominio(
                    "IMAGEN_INVALIDA",
                    "El objeto de la imagen no es un UUID válido.",
                ));
            }
            let orden = imagen.orden.unwrap_or_else(|| siguiente_orden(&ordenes));
            if !(0..MAX_IMAGENES).contains(&orden) || ordenes.contains(&orden) {
                return Err(ApiError::dominio(
                    "IMAGEN_ORDEN_INVALIDO",
                    "El orden de la imagen no es válido.",
                ));
            }
            ordenes.insert(orden);
            if imagen.es_principal {
                principales += 1;
            }
        }
        if principales > 1 {
            return Err(ApiError::dominio(
                "IMAGEN_PRINCIPAL_UNICA",
                "Solo puede haber una imagen principal.",
            ));
        }
        let mut unicos: Vec<String> = Vec::new();
        let mut vistos: HashSet<String> = HashSet::new();
        for imagen in imagenes {
            if vistos.insert(imagen.objeto_id.clone()) {
                unicos.push(imagen.objeto_id.clone());
            }
        }
        for objeto_id in unicos {
            self.validar_objeto_imagen(empresa_id, &objeto_id).await?;
        }
        Ok(())
    }

    /// ADR-28: no se puede crear/eliminar una variante ni borrar la imagen
    /// principal mientras exista una oferta activa que la referencie.
    pub(super) async fn validar_objeto_imagen(
        &self,
        empresa_id: &str,
        objeto_id: &str,
    ) -> Result<(), ApiError> {
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(4_000))
            .build()
            .map_err(|_| {
                ApiError::dominio(
                    "IMAGEN_NO_VERIFICABLE",
                    "No se pudo verificar la imagen en storage.",
                )
            })?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/storage/objetos/{}?tenant_id={}",
                self.storage_url,
                urlencoding::encode(objeto_id),
                urlencoding::encode(empresa_id)
            ))
            .header("x-internal-key", &self.clave_interna)
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio(
                    "IMAGEN_NO_VERIFICABLE",
                    "No se pudo verificar la imagen en storage.",
                )
            })?;
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ApiError::dominio(
                "IMAGEN_NO_ENCONTRADA",
                "La imagen no existe en el almacenamiento.",
            ));
        }
        if !respuesta.status().is_success() {
            return Err(ApiError::dominio(
                "IMAGEN_NO_VERIFICABLE",
                "No se pudo verificar la imagen en storage.",
            ));
        }
        let objeto = respuesta.json::<Value>().await.map_err(|_| {
            ApiError::dominio(
                "IMAGEN_NO_VERIFICABLE",
                "No se pudo verificar la imagen en storage.",
            )
        })?;
        if objeto.get("tenant_id").and_then(Value::as_str) != Some(empresa_id) {
            return Err(ApiError::dominio(
                "IMAGEN_OTRA_EMPRESA",
                "La imagen pertenece a otra empresa.",
            ));
        }
        if objeto.get("estado").and_then(Value::as_str) != Some("activo") {
            return Err(ApiError::dominio(
                "IMAGEN_NO_ACTIVA",
                "La imagen debe estar confirmada y activa.",
            ));
        }
        let content_type = objeto
            .get("content_type")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !content_type.starts_with("image/") || content_type == "image/svg+xml" {
            return Err(ApiError::dominio(
                "IMAGEN_TIPO_INVALIDO",
                "El objeto no es una imagen raster permitida.",
            ));
        }
        if objeto.get("publico").and_then(Value::as_bool) != Some(true) {
            return Err(ApiError::dominio(
                "IMAGEN_NO_PUBLICA",
                "La imagen debe subirse con publico=true para servirse en el catálogo.",
            ));
        }
        Ok(())
    }
}
