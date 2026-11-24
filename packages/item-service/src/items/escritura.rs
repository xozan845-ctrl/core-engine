use super::*;

impl Items {
    async fn resolver_negocio(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match negocio_id {
            Some(id) => {
                if !es_uuid(id) {
                    return Err(ApiError::dominio(
                        "NEGOCIO_INVALIDO",
                        "El id de negocio no es un UUID valido.",
                    ));
                }
                sqlx::query(
                    "SELECT id FROM identity.negocios
                      WHERE id = $1::uuid AND empresa_id = $2::uuid",
                )
                .bind(id)
                .bind(empresa_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?
                .map(|fila| fila.get::<Uuid, _>("id").to_string())
                .map(Some)
                .ok_or_else(|| ApiError::no_encontrado("Negocio", id))?
            }
            None => sqlx::query(
                "SELECT id FROM identity.negocios
                  WHERE empresa_id = $1::uuid AND es_predeterminado LIMIT 1",
            )
            .bind(empresa_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?
            .map(|fila| fila.get::<Uuid, _>("id").to_string()),
        };
        tx.commit().await.map_err(interno)?;
        Ok(fila)
    }

    pub async fn crear_item(&self, datos: &Value, empresa_id: &str) -> Result<Value, ApiError> {
        let tipo = exigir_tipo(datos.get("tipo").unwrap_or(&Value::Null))?;
        let codigo = normalizar_codigo(&texto(datos, "codigo"))?;
        let nombre = validar_nombre(&texto(datos, "nombre"))?;
        let precio = monto(datos, "precio")?;
        let costo = match datos.get("costo") {
            Some(Value::Null) | None => Money::desde_centavos(0)?,
            Some(_) => monto(datos, "costo")?,
        };
        let track_inventory = if tipo == "producto" {
            booleano(datos, "track_inventory").unwrap_or(true)
        } else {
            false
        };
        let stock_inicial = if track_inventory {
            let stock = entero(datos, "stock").unwrap_or(-1);
            if stock < 0 {
                return Err(ApiError::dominio(
                    "STOCK_INVALIDO",
                    "El stock debe ser un entero no negativo.",
                ));
            }
            Some(stock)
        } else {
            None
        };
        let duration_minutes = if tipo == "servicio" {
            let duracion = entero(datos, "duration_minutes").unwrap_or(0);
            if !(1..=1440).contains(&duracion) {
                return Err(ApiError::dominio(
                    "DURACION_INVALIDA",
                    "La duración debe estar entre 1 y 1440 minutos.",
                ));
            }
            Some(duracion)
        } else {
            None
        };
        if let Some(padre) = texto_opcional(datos, "parent_item_id").filter(|v| !v.is_empty()) {
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let fila = sqlx::query(
                "SELECT id, tipo, empresa_id FROM item.items WHERE id = $1::uuid AND eliminado_en IS NULL",
            )
            .bind(padre)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
            tx.commit().await.map_err(interno)?;
            let valido = fila.is_some_and(|fila| {
                let tipo_padre: String = fila.get("tipo");
                let empresa_padre: Uuid = fila.get("empresa_id");
                tipo_padre == tipo && empresa_padre.to_string() == empresa_id
            });
            if !valido {
                return Err(ApiError::dominio(
                    "ITEM_PADRE_INVALIDO",
                    "El item padre debe existir, ser del mismo tipo y de la misma empresa.",
                ));
            }
        }
        let atributos = self
            .validar_atributos(
                empresa_id,
                tipo,
                &texto(datos, "categoria"),
                datos.get("atributos").cloned().unwrap_or_else(|| json!({})),
            )
            .await?;
        self.exigir_codigo_libre(empresa_id, tipo, &codigo).await?;
        let negocio_id = self
            .resolver_negocio(empresa_id, texto_opcional(datos, "negocio_id"))
            .await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado = sqlx::query(&format!(
            "INSERT INTO item.items
               (empresa_id, tipo, codigo, nombre, descripcion, categoria,
                costo_cents, track_inventory, duration_minutes, atributos, estado,
                parent_item_id, marca, codigo_barras, unidad, permite_fracciones,
                unidad_compra, contenido_cantidad, contenido_unidad, tipo_precio,
                tipo_costo, stock_minimo, stock_maximo, garantia_meses, es_materia_prima,
                ubicacion, negocio_id)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10::jsonb, $11, $12::uuid,
                     $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26, $27::uuid)
             RETURNING {COLUMNAS_RETURNING}"
        ))
        .bind(empresa_id)
        .bind(tipo)
        .bind(&codigo)
        .bind(&nombre)
        .bind(texto(datos, "descripcion"))
        .bind({
            let categoria = texto(datos, "categoria");
            if categoria.is_empty() {
                "general".to_string()
            } else {
                categoria
            }
        })
        .bind(costo.centavos)
        .bind(track_inventory)
        .bind(duration_minutes)
        .bind(atributos.to_string())
        .bind(if texto(datos, "estado") == "inactivo" {
            "inactivo"
        } else {
            "activo"
        })
        .bind(texto_opcional(datos, "parent_item_id").filter(|v| !v.is_empty()))
        .bind(texto(datos, "marca"))
        .bind(texto(datos, "codigo_barras"))
        .bind({
            let unidad = texto(datos, "unidad");
            if unidad.is_empty() {
                "Unidad (u)".to_string()
            } else {
                unidad
            }
        })
        .bind(booleano(datos, "permite_fracciones").unwrap_or(false))
        .bind({
            let unidad = texto(datos, "unidad_compra");
            if unidad.is_empty() {
                "Igual que la de venta".to_string()
            } else {
                unidad
            }
        })
        .bind(entero(datos, "contenido_cantidad").unwrap_or(0))
        .bind({
            let contenido = texto(datos, "contenido_unidad");
            if contenido.is_empty() {
                "Sin contenido".to_string()
            } else {
                contenido
            }
        })
        .bind(if texto(datos, "tipo_precio") == "escala" {
            "escala"
        } else {
            "fijo"
        })
        .bind(if texto(datos, "tipo_costo") == "escala" {
            "escala"
        } else {
            "fijo"
        })
        .bind(entero(datos, "stock_minimo").unwrap_or(0))
        .bind(entero(datos, "stock_maximo"))
        .bind(entero(datos, "garantia_meses"))
        .bind(booleano(datos, "es_materia_prima").unwrap_or(false))
        .bind(texto(datos, "ubicacion"))
        .bind(negocio_id.as_deref())
        .fetch_optional(&mut *tx)
        .await;
        match resultado {
            Ok(fila) => {
                tx.commit().await.map_err(interno)?;
                let Some(fila) = fila else {
                    return Err(ApiError::dominio(
                        "ITEM_NO_CREADO",
                        "No se pudo crear el item.",
                    ));
                };
                let item = serializar(&fila_a_json(&fila).map_err(interno)?)?;
                if let Some(proyecciones) = &self.proyecciones {
                    let item_id = item
                        .get("item_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let resultado = async {
                        proyecciones
                            .fijar_precio(empresa_id, &item_id, precio.centavos)
                            .await?;
                        if let Some(cantidad) = stock_inicial {
                            proyecciones
                                .stock_inicial(
                                    empresa_id,
                                    &item_id,
                                    &codigo,
                                    cantidad,
                                    negocio_id.as_deref(),
                                )
                                .await?;
                        }
                        Ok::<(), ApiError>(())
                    }
                    .await;
                    if let Err(error) = resultado {
                        let mut tx = self.db.transaccion().await.map_err(interno)?;
                        sqlx::query("DELETE FROM item.items WHERE id = $1::uuid")
                            .bind(&item_id)
                            .execute(&mut *tx)
                            .await
                            .map_err(interno)?;
                        tx.commit().await.map_err(interno)?;
                        return Err(error);
                    }
                }
                Ok(item)
            }
            Err(error) => {
                let _ = tx.rollback().await;
                if es_violacion_unicidad(&error) {
                    return Err(ApiError::conflicto(format!(
                        "Ya existe un item con el código {codigo} en esta empresa."
                    )));
                }
                Err(interno(error))
            }
        }
    }

    pub async fn crear_alquiler(&self, datos: &Value, empresa_id: &str) -> Result<Value, ApiError> {
        let codigo = normalizar_codigo(&texto(datos, "codigo"))?;
        let nombre = validar_nombre(&texto(datos, "nombre"))?;
        let precio = monto(datos, "precio")?;
        let costo = match datos.get("costo") {
            Some(Value::Null) | None => Money::desde_centavos(0)?,
            Some(_) => monto(datos, "costo")?,
        };
        let atributos = self
            .validar_atributos(
                empresa_id,
                "alquiler",
                &texto(datos, "categoria"),
                datos.get("atributos").cloned().unwrap_or_else(|| json!({})),
            )
            .await?;
        self.exigir_codigo_libre(empresa_id, "alquiler", &codigo)
            .await?;
        let negocio_id = self
            .resolver_negocio(empresa_id, texto_opcional(datos, "negocio_id"))
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO item.items
               (empresa_id, tipo, codigo, nombre, descripcion, categoria,
                costo_cents, track_inventory, atributos, estado, negocio_id)
             VALUES ($1::uuid, 'alquiler', $2, $3, $4, $5, $6, false, $7::jsonb, $8, $9::uuid)
             RETURNING {COLUMNAS_RETURNING}"
        ))
        .bind(empresa_id)
        .bind(&codigo)
        .bind(&nombre)
        .bind(texto(datos, "descripcion"))
        .bind({
            let categoria = texto(datos, "categoria");
            if categoria.is_empty() {
                "general".to_string()
            } else {
                categoria
            }
        })
        .bind(costo.centavos)
        .bind(atributos.to_string())
        .bind(if texto(datos, "estado") == "inactivo" {
            "inactivo"
        } else {
            "activo"
        })
        .bind(negocio_id.as_deref())
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let Some(fila) = fila else {
            return Err(ApiError::dominio(
                "ITEM_NO_CREADO",
                "No se pudo crear el alquiler.",
            ));
        };
        let item = serializar(&fila_a_json(&fila).map_err(interno)?)?;
        if let Some(proyecciones) = &self.proyecciones {
            let item_id = item
                .get("item_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if let Err(error) = proyecciones
                .fijar_precio(empresa_id, &item_id, precio.centavos)
                .await
            {
                let mut tx = self.db.transaccion().await.map_err(interno)?;
                sqlx::query("DELETE FROM item.items WHERE id = $1::uuid")
                    .bind(&item_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(interno)?;
                tx.commit().await.map_err(interno)?;
                return Err(error);
            }
        }
        Ok(item)
    }

    pub async fn eliminar(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        if !es_uuid(id) {
            return Err(ApiError::dominio(
                "ITEM_INVALIDO",
                "El id del item no es válido.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE item.items
                SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::dominio(
                "ITEM_NO_ENCONTRADO",
                "El item no existe para esta empresa.",
            ));
        }
        Ok(())
    }

    pub async fn guardar_esquema(
        &self,
        empresa_id: Option<&str>,
        tipo: &Value,
        categoria: &str,
        esquema: &Value,
    ) -> Result<(), ApiError> {
        let tipo = exigir_tipo(tipo)?;
        let categoria = categoria.trim().to_lowercase();
        if categoria.is_empty() {
            return Err(ApiError::dominio(
                "CATEGORIA_INVALIDA",
                "La categoría es obligatoria.",
            ));
        }
        if !esquema.is_object() {
            return Err(ApiError::dominio(
                "ESQUEMA_INVALIDO",
                "El esquema debe ser un objeto JSON Schema.",
            ));
        }
        if let Err(mensaje) = compilar_esquema(esquema) {
            return Err(
                ApiError::dominio("ESQUEMA_INVALIDO", "El esquema JSON no es válido.")
                    .con_detalles(json!({ "mensaje": mensaje })),
            );
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "INSERT INTO item.item_atributo_esquemas
               (empresa_id, tipo, categoria, esquema, version, activo)
             VALUES ($1::uuid, $2, $3, $4::jsonb,
                     COALESCE((SELECT MAX(version) + 1 FROM item.item_atributo_esquemas
                                WHERE empresa_id IS NOT DISTINCT FROM $1::uuid AND tipo = $2
                                  AND LOWER(categoria) = LOWER($3)), 1),
                     true)
             ON CONFLICT DO NOTHING",
        )
        .bind(empresa_id)
        .bind(tipo)
        .bind(&categoria)
        .bind(esquema.to_string())
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    async fn exigir_codigo_libre(
        &self,
        empresa_id: &str,
        tipo: &str,
        codigo: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id FROM item.items
              WHERE empresa_id = $1::uuid AND tipo = $2 AND upper(codigo) = upper($3)
                AND eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(tipo)
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_some() {
            return Err(ApiError::conflicto(format!(
                "Ya existe un item con el código {codigo} en esta empresa."
            )));
        }
        Ok(())
    }
}
