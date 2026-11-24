use super::*;

impl Objetos {
    pub async fn eliminar(
        &self,
        tenant_id: &str,
        id: &str,
        usuario: Option<&UsuarioContexto>,
    ) -> Result<(), ApiError> {
        let fila = self.obtener_fila(tenant_id, id).await?;
        if let Some(usuario) = usuario {
            if usuario.rol != "admin" {
                if let Some(autor) = &fila.subido_por {
                    if autor != &usuario.user_id {
                        return Err(ApiError::acceso_denegado(
                            "Solo el autor de la subida o un administrador pueden eliminarlo.",
                        ));
                    }
                }
            }
        }
        if fila.estado == "eliminado" {
            return Ok(());
        }
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let objeto_id = Uuid::parse_str(id).map_err(|_| ApiError::interno())?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let eliminado = sqlx::query(&format!(
            "UPDATE storage.objetos SET estado = 'eliminado', eliminado_en = NOW()
              WHERE tenant_id = $1 AND id = $2 AND estado <> 'eliminado'
              RETURNING {COLUMNAS}"
        ))
        .bind(tenant_uuid)
        .bind(objeto_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        let Some(eliminado) = eliminado else {
            tx.commit().await.map_err(error_db)?;
            return Ok(());
        };
        if fila.estado == "activo" {
            self.buckets
                .restar_uso_en_transaccion(&mut tx, tenant_id, fila.tamano_bytes)
                .await?;
        }
        self.emitir_eliminado(&mut tx, &eliminado).await?;
        tx.commit().await.map_err(error_db)?;
        if let Err(error) = self.almacen.eliminar(&fila.bucket, &fila.objeto).await {
            logs::error(
                "storage.objetos",
                crate::campos! {
                    "msg": "No se pudo borrar el objeto fisico; queda huerfano para reconciliacion",
                    "objeto_id": id,
                    "bucket": fila.bucket,
                    "err": error.to_string(),
                },
            );
        }
        Ok(())
    }

    /// Exporta metadatos del tenant y, para objetos pequeños, su contenido en
    /// base64. Los objetos grandes se referencian por id para no generar un
    /// archivo sin límite.
    pub async fn exportar_tenant(&self, tenant_id: &str) -> Result<Value, ApiError> {
        const LIMITE_BYTES: i64 = 5 * 1024 * 1024;
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let objetos: Value = sqlx::query_scalar(
            "SELECT COALESCE(jsonb_agg(to_jsonb(o) ORDER BY o.creado_en), '[]'::jsonb)
               FROM storage.objetos o
              WHERE o.tenant_id = $1 AND o.estado <> 'eliminado'",
        )
        .bind(tenant_uuid)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        let objetos = objetos.as_array().cloned().unwrap_or_default();
        let mut salida = Vec::with_capacity(objetos.len());
        for mut objeto in objetos {
            let id = objeto
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let tamano = objeto
                .get("tamano_bytes")
                .and_then(Value::as_i64)
                .unwrap_or(0);
            if tamano > 0 && tamano <= LIMITE_BYTES {
                if let Ok(fila) = self.obtener_fila(tenant_id, &id).await {
                    if let Ok(Some(leido)) = self.almacen.leer(&fila.bucket, &fila.objeto).await {
                        let bytes = leido
                            .cuerpo
                            .collect()
                            .await
                            .map(|agregado| agregado.into_bytes())
                            .unwrap_or_default();
                        if let Some(mapa) = objeto.as_object_mut() {
                            mapa.insert(
                                "contenido_base64".to_string(),
                                Value::String(
                                    base64::engine::general_purpose::STANDARD.encode(bytes),
                                ),
                            );
                        }
                    }
                }
            } else if tamano > LIMITE_BYTES {
                if let Some(mapa) = objeto.as_object_mut() {
                    mapa.insert("omitido_por_tamano".to_string(), Value::Bool(true));
                }
            }
            salida.push(objeto);
        }
        let bucket: Value = sqlx::query_scalar(
            "SELECT COALESCE(to_jsonb(b), '{}'::jsonb)
               FROM storage.buckets b WHERE b.tenant_id = $1",
        )
        .bind(tenant_uuid)
        .fetch_one(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(json!({
            "bucket": bucket,
            "objetos": salida,
            "limite_bytes_incluidos": LIMITE_BYTES,
        }))
    }

    /// Purga todos los objetos de un tenant: marca como eliminados, resta el
    /// uso del bucket, emite un evento por objeto y borra el fichero físico.
    /// Idempotente: los objetos ya eliminados se omiten.
    pub async fn purgar_tenant(&self, tenant_id: &str) -> Result<Value, ApiError> {
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM storage.objetos
              WHERE tenant_id = $1 AND estado <> 'eliminado'"
        ))
        .bind(tenant_uuid)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        if filas.is_empty() {
            return Ok(json!({ "objetos": 0, "bytes": 0, "eliminados": 0, "errores": 0 }));
        }
        let objetos: Vec<ObjetoStorage> = filas
            .iter()
            .map(serializar)
            .collect::<Result<Vec<_>, _>>()?;
        let bytes: i64 = objetos
            .iter()
            .filter(|objeto| objeto.estado == "activo")
            .map(|objeto| objeto.tamano_bytes)
            .sum();
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        for objeto in &objetos {
            let actualizado = sqlx::query(&format!(
                "UPDATE storage.objetos SET estado = 'eliminado', eliminado_en = NOW()
                  WHERE id = $1 AND estado <> 'eliminado'
                  RETURNING {COLUMNAS}"
            ))
            .bind(Uuid::parse_str(&objeto.id).map_err(|_| ApiError::interno())?)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?;
            if let Some(fila) = actualizado {
                self.emitir_eliminado(&mut tx, &fila).await?;
            }
        }
        if bytes > 0 {
            self.buckets
                .restar_uso_en_transaccion(&mut tx, tenant_id, bytes)
                .await?;
        }
        tx.commit().await.map_err(error_db)?;

        let mut errores = 0usize;
        for objeto in &objetos {
            if let Err(error) = self.almacen.eliminar(&objeto.bucket, &objeto.objeto).await {
                errores += 1;
                logs::error(
                    "storage.objetos",
                    crate::campos! {
                        "msg": "No se pudo borrar el objeto fisico durante la purga",
                        "objeto_id": objeto.id,
                        "bucket": objeto.bucket,
                        "err": error.to_string(),
                    },
                );
            }
        }
        Ok(json!({
            "objetos": objetos.len(),
            "bytes": bytes,
            "eliminados": objetos.len().saturating_sub(errores),
            "errores": errores,
        }))
    }

    async fn emitir_eliminado(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        fila: &PgRow,
    ) -> Result<(), ApiError> {
        let objeto = serializar(fila)?;
        let data = json!({
            "objeto_id": objeto.id,
            "tenant_id": objeto.tenant_id,
            "bucket": objeto.bucket,
            "objeto": objeto.objeto,
            "nombre": objeto.nombre,
            "tamano_bytes": objeto.tamano_bytes,
        });
        self.outbox
            .insertar_en_transaccion(tx, STORAGE_OBJETO_ELIMINADO, data)
            .await?;
        Ok(())
    }
}
