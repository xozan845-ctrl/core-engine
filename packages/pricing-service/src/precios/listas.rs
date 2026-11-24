use super::*;

impl Precios {
    pub async fn listas(&self, empresa_id: &str) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_LISTA}
               FROM pricing.listas_precio
              WHERE empresa_id = $1::uuid
              ORDER BY predeterminada DESC, nombre"
        ))
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas_json(&filas)
    }

    pub async fn crear_lista(&self, empresa_id: &str, input: &Value) -> Result<Value, ApiError> {
        let codigo = validar_codigo(&texto(input, "codigo"), "codigo")?;
        let nombre = validar_nombre(&texto(input, "nombre"))?;
        let descripcion = recortar(texto(input, "descripcion"), 500);
        let moneda = match input.get("moneda") {
            None | Some(Value::Null) => "NIO".to_string(),
            Some(valor) => texto_js(valor),
        };
        let moneda = validar_moneda(&moneda)?;
        let segmento = validar_segmento(&texto(input, "segmento_codigo"))?;
        let predeterminada = input
            .get("predeterminada")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            if predeterminada {
                sqlx::query(
                    "UPDATE pricing.listas_precio
                        SET predeterminada = false, actualizado_en = NOW()
                      WHERE empresa_id = $1::uuid AND predeterminada",
                )
                .bind(empresa_id)
                .execute(&mut *tx)
                .await?;
            }
            let fila = sqlx::query(&format!(
                "INSERT INTO pricing.listas_precio
                   (empresa_id, codigo, nombre, descripcion, moneda, segmento_codigo, predeterminada)
                 VALUES ($1::uuid, $2, $3, $4, $5, $6, $7)
                 RETURNING {COLUMNAS_LISTA}"
            ))
            .bind(empresa_id)
            .bind(&codigo)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(&moneda)
            .bind(&segmento)
            .bind(predeterminada)
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                Err(ApiError::dominio(
                    "LISTA_NO_CREADA",
                    "No se pudo crear la lista.",
                ))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error, MENSAJE_CONFLICTO_LISTA))
            }
        }
    }

    pub async fn actualizar_lista(
        &self,
        empresa_id: &str,
        lista_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar_lista(empresa_id, lista_id).await?;

        let codigo = match input.get("codigo") {
            Some(valor) => validar_codigo(&texto_js(valor), "codigo")?,
            None => texto_de(&actual, "codigo"),
        };
        let nombre = match input.get("nombre") {
            Some(valor) => validar_nombre(&texto_js(valor))?,
            None => texto_de(&actual, "nombre"),
        };
        let descripcion = match input.get("descripcion") {
            Some(valor) => recortar(texto_js(valor).trim().to_string(), 500),
            None => texto_de(&actual, "descripcion"),
        };
        let moneda = match input.get("moneda") {
            Some(valor) => validar_moneda(&texto_js(valor))?,
            None => texto_de(&actual, "moneda"),
        };
        let segmento = match input.get("segmento_codigo") {
            Some(valor) => validar_segmento(&texto_js(valor))?,
            None => texto_de(&actual, "segmento_codigo"),
        };
        let activo_actual = booleano_de(&actual, "activo");
        let activo = match input.get("activo") {
            Some(valor) => valor.as_bool().unwrap_or(false),
            None => activo_actual,
        };
        let predeterminada_actual = booleano_de(&actual, "predeterminada");
        let predeterminada = match input.get("predeterminada") {
            Some(valor) => valor.as_bool().unwrap_or(false),
            None => predeterminada_actual,
        } && activo;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            if predeterminada && !predeterminada_actual {
                sqlx::query(
                    "UPDATE pricing.listas_precio
                        SET predeterminada = false, actualizado_en = NOW()
                      WHERE empresa_id = $1::uuid AND predeterminada AND id <> $2",
                )
                .bind(empresa_id)
                .bind(lista_id)
                .execute(&mut *tx)
                .await?;
            }
            let fila = sqlx::query(&format!(
                "UPDATE pricing.listas_precio
                    SET codigo = $1, nombre = $2, descripcion = $3, moneda = $4,
                        segmento_codigo = $5, predeterminada = $6, activo = $7,
                        actualizado_en = NOW()
                  WHERE id = $8::uuid AND empresa_id = $9::uuid
                  RETURNING {COLUMNAS_LISTA}"
            ))
            .bind(&codigo)
            .bind(&nombre)
            .bind(&descripcion)
            .bind(&moneda)
            .bind(&segmento)
            .bind(predeterminada)
            .bind(activo)
            .bind(lista_id)
            .bind(empresa_id)
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                Err(ApiError::no_encontrado("Lista de precios", lista_id))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error, MENSAJE_CONFLICTO_LISTA))
            }
        }
    }

    // ── precios de una lista ───────────────────────────────────────────────

    pub async fn precios(&self, empresa_id: &str, lista_id: &str) -> Result<Value, ApiError> {
        self.buscar_lista(empresa_id, lista_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS_PRECIO}
               FROM pricing.lista_precio_items
              WHERE empresa_id = $1::uuid AND lista_id = $2::uuid
              ORDER BY item_id, desde_cantidad"
        ))
        .bind(empresa_id)
        .bind(lista_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas_json(&filas)
    }

    pub(super) async fn resolver_lista(
        &self,
        empresa_id: &str,
        filtros: &Value,
    ) -> Result<Option<Value>, ApiError> {
        if let Some(codigo) = filtro_texto(filtros, "lista_codigo") {
            let codigo = validar_codigo(&codigo, "lista_codigo")?;
            let fila = self
                .consulta_lista(
                    &format!(
                        "SELECT {COLUMNAS_LISTA}
                           FROM pricing.listas_precio
                          WHERE empresa_id = $1::uuid AND lower(codigo) = $2 AND activo"
                    ),
                    empresa_id,
                    &codigo,
                )
                .await?;
            let fila = fila.ok_or_else(|| ApiError::no_encontrado("Lista de precios", &codigo))?;
            return Ok(Some(resumen_lista(&fila)));
        }
        if let Some(segmento) = filtro_texto(filtros, "segmento_codigo") {
            let segmento = validar_segmento(&segmento)?;
            let fila = self
                .consulta_lista(
                    &format!(
                        "SELECT {COLUMNAS_LISTA}
                           FROM pricing.listas_precio
                          WHERE empresa_id = $1::uuid AND segmento_codigo = $2 AND activo
                          ORDER BY predeterminada DESC, creado_en
                          LIMIT 1"
                    ),
                    empresa_id,
                    &segmento,
                )
                .await?;
            let fila = fila.ok_or_else(|| {
                ApiError::no_encontrado("Lista de precios del segmento", &segmento)
            })?;
            return Ok(Some(resumen_lista(&fila)));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_LISTA}
               FROM pricing.listas_precio
              WHERE empresa_id = $1::uuid AND predeterminada AND activo
              LIMIT 1"
        ))
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(Some(resumen_lista(&fila_json(&fila)?))),
            None => Ok(None),
        }
    }

    async fn consulta_lista(
        &self,
        sql: &str,
        empresa_id: &str,
        valor: &str,
    ) -> Result<Option<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(sql)
            .bind(empresa_id)
            .bind(valor)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(Some(fila_json(&fila)?)),
            None => Ok(None),
        }
    }

    pub(super) async fn buscar_lista(
        &self,
        empresa_id: &str,
        lista_id: &str,
    ) -> Result<Value, ApiError> {
        let id = validar_uuid(lista_id, "lista")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS_LISTA}
               FROM pricing.listas_precio
              WHERE id = $1::uuid AND empresa_id = $2::uuid"
        ))
        .bind(&id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_json(&fila),
            None => Err(ApiError::no_encontrado("Lista de precios", &id)),
        }
    }
}
