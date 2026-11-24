use super::*;

impl Objetos {
    pub async fn listar(
        &self,
        tenant_id: &str,
        filtros: &FiltrosObjetos,
    ) -> Result<Pagina<ObjetoStorage>, ApiError> {
        let (pagina, limite) = parsear_paginacion(filtros.pagina, filtros.limite);
        let tenant_uuid = Uuid::parse_str(tenant_id).map_err(|_| ApiError::interno())?;

        let mut count = QueryBuilder::<Postgres>::new(
            "SELECT COUNT(*) FROM storage.objetos WHERE tenant_id = ",
        );
        count.push_bind(tenant_uuid);
        aplicar_filtros(&mut count, filtros);
        let total: i64 = count
            .build_query_scalar()
            .fetch_one(self.db.pool())
            .await
            .map_err(error_db)?;

        let mut query = QueryBuilder::<Postgres>::new(&format!(
            "SELECT {COLUMNAS} FROM storage.objetos WHERE tenant_id = "
        ));
        query.push_bind(tenant_uuid);
        aplicar_filtros(&mut query, filtros);
        query.push(" ORDER BY creado_en DESC LIMIT ");
        query.push_bind(limite);
        query.push(" OFFSET ");
        query.push_bind((pagina - 1) * limite);
        let filas = query
            .build()
            .fetch_all(self.db.pool())
            .await
            .map_err(error_db)?;
        let items = filas
            .iter()
            .map(serializar)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crear_pagina(items, total, pagina, limite))
    }

    pub async fn obtener(&self, tenant_id: &str, id: &str) -> Result<ObjetoStorage, ApiError> {
        self.obtener_fila(tenant_id, id).await
    }

    pub async fn url_descarga(&self, tenant_id: &str, id: &str) -> Result<UrlDescarga, ApiError> {
        let fila = self.obtener_fila(tenant_id, id).await?;
        if fila.estado == "eliminado" {
            return Err(ApiError::no_encontrado("Objeto", id));
        }
        let url_descarga = self
            .almacen
            .url_descarga(&fila.bucket, &fila.objeto, self.config.url_ttl_segundos)
            .await
            .map_err(error_almacen)?;
        Ok(UrlDescarga {
            url_descarga,
            expira_en: self.expiracion(),
            nombre: fila.nombre,
            content_type: fila.content_type,
            tamano_bytes: fila.tamano_bytes,
        })
    }

    pub async fn leer_publico(
        &self,
        id: &str,
        rango: Option<(i64, i64)>,
    ) -> Result<ObjetoPublico, ApiError> {
        let fila = self.fila_publica(id).await?;
        let leido = match rango {
            Some((inicio, fin)) => {
                self.almacen
                    .leer_rango(&fila.bucket, &fila.objeto, inicio, fin)
                    .await
            }
            None => self.almacen.leer(&fila.bucket, &fila.objeto).await,
        }
        .map_err(error_almacen)?
        .ok_or_else(|| ApiError::no_encontrado("Objeto", id))?;
        let etag = leido.etag.clone().or(fila.etag.clone());
        Ok(ObjetoPublico {
            cuerpo: leido,
            content_type: fila.content_type,
            tamano_bytes: fila.tamano_bytes,
            etag,
            nombre: fila.nombre,
        })
    }

    pub async fn obtener_publico_meta(&self, id: &str) -> Result<ObjetoPublicoMeta, ApiError> {
        let fila = self.fila_publica(id).await?;
        Ok(ObjetoPublicoMeta {
            content_type: fila.content_type,
            tamano_bytes: fila.tamano_bytes,
            etag: fila.etag,
            nombre: fila.nombre,
        })
    }

    /// Lee el contenido de un objeto del tenant (servicio→servicio).
    pub async fn leer_contenido(
        &self,
        tenant_id: &str,
        id: &str,
    ) -> Result<ObjetoPublico, ApiError> {
        let fila = self.obtener_fila(tenant_id, id).await?;
        if fila.estado != "activo" {
            return Err(ApiError::no_encontrado("Objeto", id));
        }
        let leido = self
            .almacen
            .leer(&fila.bucket, &fila.objeto)
            .await
            .map_err(error_almacen)?
            .ok_or_else(|| ApiError::no_encontrado("Objeto", id))?;
        let etag = leido.etag.clone().or(fila.etag.clone());
        Ok(ObjetoPublico {
            cuerpo: leido,
            content_type: fila.content_type,
            tamano_bytes: fila.tamano_bytes,
            etag,
            nombre: fila.nombre,
        })
    }

    async fn fila_publica(&self, id: &str) -> Result<ObjetoStorage, ApiError> {
        let uuid = Uuid::parse_str(id).map_err(|_| ApiError::no_encontrado("Objeto", id))?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM storage.objetos
              WHERE id = $1 AND estado = 'activo' AND publico = true"
        ))
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Objeto", id))?;
        let objeto = serializar(&fila)?;
        if !es_content_type_publico(&objeto.content_type) {
            return Err(ApiError::no_encontrado("Objeto", id));
        }
        Ok(objeto)
    }
}
