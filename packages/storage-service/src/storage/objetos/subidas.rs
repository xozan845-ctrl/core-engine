use super::*;

impl Objetos {
    pub async fn solicitar_subida(
        &self,
        tenant_id: &str,
        usuario_id: Option<&str>,
        datos: DatosSubida,
    ) -> Result<SolicitudSubida, ApiError> {
        let nombre = sanear_nombre(&datos.nombre);
        let content_type = datos.content_type.trim().to_lowercase();
        if !self.config.content_types_permitidos.contains(&content_type) {
            return Err(ApiError::dominio(
                "CONTENT_TYPE_NO_PERMITIDO",
                format!("El tipo \"{content_type}\" no esta permitido."),
            )
            .con_detalles(json!({ "permitidos": self.config.content_types_permitidos })));
        }
        if datos.tamano_bytes <= 0 {
            return Err(ApiError::dominio(
                "TAMANO_INVALIDO",
                "El tamano debe ser un entero positivo en bytes.",
            ));
        }
        let max_bytes = self.maximo_para(&content_type);
        if datos.tamano_bytes > max_bytes {
            return Err(ApiError::dominio(
                "OBJETO_DEMASIADO_GRANDE",
                format!("El archivo excede el maximo de {max_bytes} bytes."),
            )
            .con_detalles(json!({ "max_bytes": max_bytes })));
        }

        let estado = self.buckets.asegurar(tenant_id).await?;
        self.buckets.verificar_cuota(&estado, datos.tamano_bytes)?;

        let id = Uuid::new_v4();
        let carpeta = sanear_carpeta(datos.carpeta.as_deref());
        let objeto = format!("{carpeta}/{id}/{nombre}");
        let etiquetas = datos.etiquetas.unwrap_or_default();
        let autor = usuario_id.filter(|valor| Uuid::parse_str(valor).is_ok());
        let descripcion = datos
            .descripcion
            .unwrap_or_default()
            .chars()
            .take(500)
            .collect::<String>();

        sqlx::query(
            "INSERT INTO storage.objetos
               (id, tenant_id, bucket, objeto, nombre_original, content_type, carpeta,
                descripcion, etiquetas, tamano_bytes, estado, subido_por, publico, creado_en)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::jsonb, $10, 'pendiente', $11, $12, NOW())",
        )
        .bind(id)
        .bind(Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?)
        .bind(&estado.bucket)
        .bind(&objeto)
        .bind(&nombre)
        .bind(&content_type)
        .bind(&carpeta)
        .bind(&descripcion)
        .bind(sqlx::types::Json(&etiquetas))
        .bind(datos.tamano_bytes)
        .bind(autor.map(|valor| Uuid::parse_str(valor).expect("uuid validado")))
        .bind(datos.publico)
        .execute(self.db.pool())
        .await
        .map_err(error_db)?;

        let url_subida = self
            .almacen
            .url_subida(
                &estado.bucket,
                &objeto,
                &content_type,
                self.config.url_ttl_segundos,
            )
            .await
            .map_err(error_almacen)?;
        let mut headers = BTreeMap::new();
        headers.insert("Content-Type".to_string(), content_type);
        Ok(SolicitudSubida {
            objeto_id: id.to_string(),
            bucket: estado.bucket,
            objeto,
            url_subida,
            metodo: "PUT".to_string(),
            headers,
            expira_en: self.expiracion(),
            max_bytes,
        })
    }

    /// Subida directa servicio→servicio (PDFs de comprobantes). Reutiliza la
    #[allow(clippy::too_many_arguments)]
    /// validación, cuota y outbox del flujo prefirmado.
    pub async fn subir_contenido(
        &self,
        tenant_id: &str,
        usuario_id: Option<&str>,
        nombre: &str,
        content_type: &str,
        bytes: Vec<u8>,
        publico: bool,
        descripcion: Option<String>,
    ) -> Result<ObjetoStorage, ApiError> {
        let tamano = i64::try_from(bytes.len()).map_err(|_| ApiError::interno())?;
        if tamano <= 0 {
            return Err(ApiError::dominio(
                "OBJETO_VACIO",
                "El contenido no puede estar vacío.",
            ));
        }
        let solicitud = self
            .solicitar_subida(
                tenant_id,
                usuario_id,
                DatosSubida {
                    nombre: nombre.to_string(),
                    content_type: content_type.to_string(),
                    tamano_bytes: tamano,
                    publico,
                    descripcion,
                    ..Default::default()
                },
            )
            .await?;
        if let Err(error) = self
            .almacen
            .subir(&solicitud.bucket, &solicitud.objeto, bytes, content_type)
            .await
        {
            let _ = sqlx::query("DELETE FROM storage.objetos WHERE id = $1")
                .bind(Uuid::parse_str(&solicitud.objeto_id).map_err(|_| ApiError::interno())?)
                .execute(self.db.pool())
                .await;
            return Err(error_almacen(error));
        }
        self.confirmar(tenant_id, &solicitud.objeto_id).await
    }

    pub async fn confirmar(&self, tenant_id: &str, id: &str) -> Result<ObjetoStorage, ApiError> {
        let fila = self.obtener_fila(tenant_id, id).await?;
        if fila.estado == "eliminado" {
            return Err(ApiError::conflicto(
                "El objeto fue eliminado y no puede confirmarse.",
            ));
        }
        if fila.estado == "activo" {
            return Ok(fila);
        }
        let info = self
            .almacen
            .estadisticas(&fila.bucket, &fila.objeto)
            .await
            .map_err(error_almacen)?
            .ok_or_else(|| ApiError::no_encontrado("Objeto en el almacenamiento", &fila.objeto))?;
        if info.tamano_bytes <= 0 {
            let _ = self.almacen.eliminar(&fila.bucket, &fila.objeto).await;
            return Err(ApiError::dominio(
                "OBJETO_VACIO",
                "El objeto subido esta vacio.",
            ));
        }
        let content_type = info
            .content_type
            .clone()
            .unwrap_or_else(|| fila.content_type.clone())
            .trim()
            .to_lowercase();
        if !self.config.content_types_permitidos.contains(&content_type) {
            let _ = self.almacen.eliminar(&fila.bucket, &fila.objeto).await;
            return Err(ApiError::dominio(
                "CONTENT_TYPE_NO_PERMITIDO",
                format!("El objeto subido tiene un tipo no permitido (\"{content_type}\")."),
            )
            .con_detalles(json!({ "permitidos": self.config.content_types_permitidos })));
        }
        let max_bytes = self.maximo_para(&content_type);
        if info.tamano_bytes > max_bytes {
            let _ = self.almacen.eliminar(&fila.bucket, &fila.objeto).await;
            return Err(ApiError::dominio(
                "OBJETO_DEMASIADO_GRANDE",
                format!("El archivo excede el maximo de {max_bytes} bytes."),
            )
            .con_detalles(json!({ "max_bytes": max_bytes })));
        }

        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let objeto_id = Uuid::parse_str(id).map_err(|_| ApiError::interno())?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let estado = self
            .buckets
            .bloquear_estado_en_transaccion(&mut tx, tenant_id)
            .await?;
        self.buckets
            .verificar_cuota(&estado, (info.tamano_bytes - fila.tamano_bytes).max(0))?;
        let actualizado = sqlx::query(&format!(
            "UPDATE storage.objetos
                SET estado = 'activo', tamano_bytes = $3, content_type = $4, etag = $5,
                    confirmado_en = NOW()
              WHERE tenant_id = $1 AND id = $2 AND estado = 'pendiente'
              RETURNING {COLUMNAS}"
        ))
        .bind(tenant_uuid)
        .bind(objeto_id)
        .bind(info.tamano_bytes)
        .bind(&content_type)
        .bind(info.etag.clone())
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(actualizado) = actualizado else {
            tx.commit().await.map_err(error_db)?;
            return self.obtener(tenant_id, id).await;
        };
        self.buckets
            .sumar_uso_en_transaccion(&mut tx, tenant_id, info.tamano_bytes)
            .await?;
        self.emitir_subido(&mut tx, &actualizado).await?;
        tx.commit().await.map_err(error_db)?;
        serializar(&actualizado)
    }

    async fn emitir_subido(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        fila: &PgRow,
    ) -> Result<(), ApiError> {
        let objeto = serializar(fila)?;
        let mut data = Map::new();
        data.insert("objeto_id".into(), json!(objeto.id));
        data.insert("tenant_id".into(), json!(objeto.tenant_id));
        data.insert("bucket".into(), json!(objeto.bucket));
        data.insert("objeto".into(), json!(objeto.objeto));
        data.insert("nombre".into(), json!(objeto.nombre));
        data.insert("content_type".into(), json!(objeto.content_type));
        data.insert("tamano_bytes".into(), json!(objeto.tamano_bytes));
        if let Some(autor) = objeto.subido_por {
            data.insert("subido_por".into(), json!(autor));
        }
        self.outbox
            .insertar_en_transaccion(tx, STORAGE_OBJETO_SUBIDO, Value::Object(data))
            .await?;
        Ok(())
    }

    fn maximo_para(&self, content_type: &str) -> i64 {
        self.config.maximo_para(content_type)
    }
}
