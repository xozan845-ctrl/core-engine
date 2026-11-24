use super::*;

impl Tiendas {
    pub async fn de_vendedor(&self, vendedor_id: &str) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {CAMPOS_TIENDA}
             FROM stores.tiendas
             WHERE vendedor_id = $1::uuid OR EXISTS (
               SELECT 1 FROM identity.asignaciones_personal ap
               WHERE ap.usuario_id = $1::uuid AND ap.tienda_id = stores.tiendas.id AND ap.estado = 'activa'
             )
             ORDER BY CASE WHEN vendedor_id = $1::uuid THEN 0 ELSE 1 END, creado_en
             LIMIT 1"
        ))
        .bind(vendedor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        match fila {
            Some(fila) => Ok(Some(
                self.con_urls(&fila_a_json(&fila).map_err(crate::db::interno)?)
                    .await?,
            )),
            None => Ok(None),
        }
    }

    pub async fn de_empresa(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {CAMPOS_TIENDA}
             FROM stores.tiendas WHERE empresa_id = $1::uuid
               AND ($2::uuid IS NULL OR negocio_id = $2::uuid)
             ORDER BY creado_en DESC LIMIT 200"
        ))
        .bind(empresa_id)
        .bind(negocio_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let mut salida = Vec::with_capacity(filas.len());
        for fila in filas {
            let valor = fila_a_json(&fila).map_err(crate::db::interno)?;
            salida.push(self.con_urls(&valor).await?);
        }
        Ok(salida)
    }

    /// Resuelve por UUID o slug; conserva el slug anterior como redirección.
    pub async fn resolver(&self, identificador: &str) -> Result<Option<TiendaResuelta>, ApiError> {
        let Some(fila) = self.buscar_por_identificador(identificador).await? else {
            return Ok(None);
        };
        let es_actual = booleano(&fila, "es_actual");
        let overrides = overrides_de(&fila);
        Ok(Some(TiendaResuelta {
            tienda: self.con_urls(&fila).await?,
            overrides,
            redirigir_a: if es_actual {
                None
            } else {
                Some(format!("/api/v2/tiendas/{}", texto(&fila, "slug")))
            },
        }))
    }

    pub async fn encontrar_por_id(&self, id: &str) -> Result<Option<Value>, ApiError> {
        Ok(self.resolver(id).await?.map(|resuelta| resuelta.tienda))
    }

    /// Lote interno por ids (identity no debe leer `stores.tiendas`).
    pub async fn por_ids(&self, ids: &[String]) -> Result<Vec<Value>, ApiError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = ids
            .iter()
            .map(|id| Uuid::parse_str(id.trim()).map_err(crate::db::interno))
            .collect::<Result<Vec<_>, _>>()?;
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {CAMPOS_TIENDA} FROM stores.tiendas WHERE id = ANY($1::uuid[])"
        ))
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        let mut salida = Vec::with_capacity(filas.len());
        for fila in filas {
            let valor = fila_a_json(&fila).map_err(crate::db::interno)?;
            salida.push(self.con_urls(&valor).await?);
        }
        Ok(salida)
    }

    async fn buscar_por_identificador(
        &self,
        identificador: &str,
    ) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(crate::db::interno)?;
        let fila = if slug::es_uuid(identificador) {
            sqlx::query("SELECT t.*, true AS es_actual FROM stores.tiendas t WHERE t.id = $1::uuid")
                .bind(identificador)
                .fetch_optional(&mut *tx)
                .await
        } else {
            sqlx::query(
                "SELECT t.*, (s.reemplazado_en IS NULL) AS es_actual
                   FROM stores.tienda_slugs s
                   JOIN stores.tiendas t ON t.id = s.tienda_id
                  WHERE s.slug = $1",
            )
            .bind(identificador)
            .fetch_optional(&mut *tx)
            .await
        }
        .map_err(crate::db::interno)?;
        tx.commit().await.map_err(crate::db::interno)?;
        fila.map(|fila| fila_a_json(&fila).map_err(crate::db::interno))
            .transpose()
    }
}
