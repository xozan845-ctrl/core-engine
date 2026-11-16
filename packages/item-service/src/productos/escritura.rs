use super::helpers::*;
use super::*;

impl Productos {
    pub async fn crear(&self, datos: &Value, opciones: Option<&Value>) -> Result<Value, ApiError> {
        let empresa_id = texto(datos, "empresa_id");
        let sku = normalizar_sku(&texto(datos, "sku"))?;
        let stock = validar_stock(datos.get("stock"))?;
        let precio = validar_precio(datos.get("precio_base"))?;
        let metadatos = normalizar_metadatos(datos)?;
        let variantes = variantes_de(datos);
        if variantes.len() > MAX_VARIANTES {
            return Err(ApiError::dominio(
                "DEMASIADAS_VARIANTES",
                format!("Un producto admite como máximo {MAX_VARIANTES} variantes."),
            ));
        }
        let mut skus: HashSet<String> = HashSet::new();
        skus.insert(sku.clone());
        for variante in &variantes {
            let variante_sku = normalizar_sku(&variante.sku)?;
            validar_stock(variante.stock.as_ref())?;
            validar_precio(variante.precio_base.as_ref())?;
            if !skus.insert(variante_sku.clone()) {
                return Err(ApiError::conflicto(format!(
                    "El SKU {variante_sku} está repetido en la solicitud."
                )));
            }
        }
        if self.sku_existe(&empresa_id, &sku).await? {
            return Err(ApiError::conflicto(format!(
                "Ya existe un producto con el SKU {sku} en esta empresa."
            )));
        }
        for variante in &variantes {
            let variante_sku = normalizar_sku(&variante.sku)?;
            if self.sku_existe(&empresa_id, &variante_sku).await? {
                return Err(ApiError::conflicto(format!(
                    "Ya existe un producto con el SKU {variante_sku} en esta empresa."
                )));
            }
        }
        let empresa_validada = opciones
            .and_then(|opciones| {
                opciones
                    .get("empresa_validada")
                    .or_else(|| opciones.get("empresaValidada"))
            })
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !empresa_validada {
            self.validar_empresa_catalogo(&empresa_id).await?;
        }
        let categoria = categoria_de(datos);
        if let Some(items) = &self.items {
            items
                .validar_atributos(&empresa_id, "producto", &categoria, atributos_de(datos))
                .await?;
        }
        self.validar_codigo_barras(&empresa_id, &metadatos.codigo_barras, None)
            .await?;
        let imagenes = imagenes_de(datos.get("imagenes"));
        self.validar_imagenes(&empresa_id, &imagenes).await?;
        for variante in &variantes {
            if let Some(items) = &self.items {
                items
                    .validar_atributos(
                        &empresa_id,
                        "producto",
                        &categoria,
                        variante.atributos.clone(),
                    )
                    .await?;
            }
            self.validar_imagenes(&empresa_id, &variante.imagenes)
                .await?;
        }

        let padre_id = Uuid::new_v4();
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "INSERT INTO item.items
               (id, empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
                costo_cents, marca, codigo_barras, unidad, permite_fracciones,
                unidad_compra, contenido_cantidad, contenido_unidad, tipo_precio,
                tipo_costo, stock_minimo, stock_maximo, garantia_meses,
                es_materia_prima, ubicacion, estado, track_inventory, stock,
                parent_item_id, atributos, actualizado_en)
             VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13,
                     $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, true, $24,
                     NULL, $25::jsonb, NOW())",
        )
        .bind(padre_id)
        .bind(&empresa_id)
        .bind(&sku)
        .bind(texto(datos, "nombre").trim().to_string())
        .bind(texto(datos, "descripcion").trim().to_string())
        .bind(categoria.trim().to_string())
        .bind(precio.centavos)
        .bind(metadatos.costo_cents)
        .bind(&metadatos.marca)
        .bind(&metadatos.codigo_barras)
        .bind(&metadatos.unidad)
        .bind(metadatos.permite_fracciones)
        .bind(&metadatos.unidad_compra)
        .bind(metadatos.contenido_cantidad)
        .bind(&metadatos.contenido_unidad)
        .bind(&metadatos.tipo_precio)
        .bind(&metadatos.tipo_costo)
        .bind(metadatos.stock_minimo)
        .bind(metadatos.stock_maximo)
        .bind(metadatos.garantia_meses)
        .bind(metadatos.es_materia_prima)
        .bind(&metadatos.ubicacion)
        .bind(&metadatos.estado_operativo)
        .bind(stock)
        .bind(atributos_de(datos).to_string())
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        registrar_historico(
            &mut tx,
            &empresa_id,
            &padre_id.to_string(),
            None,
            precio.centavos,
        )
        .await?;

        for variante in &variantes {
            let variante_id = Uuid::new_v4();
            let variante_sku = normalizar_sku(&variante.sku)?;
            let variante_precio = validar_precio(variante.precio_base.as_ref())?;
            let variante_stock = validar_stock(variante.stock.as_ref())?;
            let nombre = variante
                .nombre
                .clone()
                .unwrap_or_else(|| texto(datos, "nombre"))
                .trim()
                .to_string();
            sqlx::query(
                "INSERT INTO item.items
                   (id, empresa_id, tipo, codigo, nombre, descripcion, categoria,
                    precio_cents, track_inventory, stock, parent_item_id, atributos)
                 VALUES ($1::uuid, $2::uuid, 'producto', $3, $4, $5, $6, $7, true, $8, $9::uuid, $10::jsonb)",
            )
            .bind(variante_id)
            .bind(&empresa_id)
            .bind(&variante_sku)
            .bind(nombre)
            .bind(texto(datos, "descripcion").trim().to_string())
            .bind(categoria.trim().to_string())
            .bind(variante_precio.centavos)
            .bind(variante_stock)
            .bind(padre_id)
            .bind(variante.atributos.to_string())
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
            registrar_historico(
                &mut tx,
                &empresa_id,
                &variante_id.to_string(),
                None,
                variante_precio.centavos,
            )
            .await?;
            insertar_imagenes(&mut tx, &empresa_id, variante_id, &variante.imagenes).await?;
        }

        insertar_imagenes(&mut tx, &empresa_id, padre_id, &imagenes).await?;
        if !variantes.is_empty() {
            recalcular_padre(&mut tx, padre_id).await?;
        }
        tx.commit().await.map_err(interno)?;

        self.encontrar_por_id(&padre_id.to_string(), Some(&empresa_id), true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", &padre_id.to_string()))
    }

    /// Agrega una variante a un producto padre (ADR-28).
    pub async fn actualizar(
        &self,
        id: &str,
        cambios: &Value,
        empresa_id: Option<&str>,
    ) -> Result<Value, ApiError> {
        let previo = self
            .encontrar_por_id(id, empresa_id, true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", id))?;
        let tiene_variantes = previo
            .get("tiene_variantes")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if tiene_variantes
            && (cambios.get("precio_base").is_some() || cambios.get("stock").is_some())
        {
            return Err(ApiError::dominio(
                "PRODUCTO_CON_VARIANTES",
                "El precio y el stock de un producto con variantes se editan en cada variante.",
            ));
        }
        let metadatos = combinar_metadatos(&previo, cambios)?;
        if cambios.get("atributos").is_some() {
            let categoria = cambios
                .get("categoria")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    previo
                        .get("categoria")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default();
            if let Some(items) = &self.items {
                items
                    .validar_atributos(
                        &texto(&previo, "empresa_id"),
                        "producto",
                        &categoria,
                        cambios.get("atributos").cloned().unwrap_or(Value::Null),
                    )
                    .await?;
            }
        }
        self.validar_codigo_barras(
            &texto(&previo, "empresa_id"),
            &metadatos.codigo_barras,
            Some(id),
        )
        .await?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut sets: Vec<String> = Vec::new();
        let mut params: Vec<Param> = Vec::new();
        if let Some(nombre) = cambios.get("nombre").and_then(Value::as_str) {
            agregar_set(
                &mut sets,
                &mut params,
                "nombre",
                Param::Texto(nombre.trim().to_string()),
            );
        }
        if let Some(descripcion) = cambios.get("descripcion").and_then(Value::as_str) {
            agregar_set(
                &mut sets,
                &mut params,
                "descripcion",
                Param::Texto(descripcion.trim().to_string()),
            );
        }
        if let Some(categoria) = cambios.get("categoria").and_then(Value::as_str) {
            agregar_set(
                &mut sets,
                &mut params,
                "categoria",
                Param::Texto(categoria.trim().to_string()),
            );
        }

        agregar_set(
            &mut sets,
            &mut params,
            "costo_cents",
            Param::Entero(metadatos.costo_cents),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "marca",
            Param::Texto(metadatos.marca),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "codigo_barras",
            Param::Texto(metadatos.codigo_barras),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "unidad",
            Param::Texto(metadatos.unidad),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "permite_fracciones",
            Param::Booleano(metadatos.permite_fracciones),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "unidad_compra",
            Param::Texto(metadatos.unidad_compra),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "contenido_cantidad",
            Param::Entero(metadatos.contenido_cantidad),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "contenido_unidad",
            Param::Texto(metadatos.contenido_unidad),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "tipo_precio",
            Param::Texto(metadatos.tipo_precio),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "tipo_costo",
            Param::Texto(metadatos.tipo_costo),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "stock_minimo",
            Param::Entero(metadatos.stock_minimo),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "stock_maximo",
            Param::OpcionalEntero(metadatos.stock_maximo),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "garantia_meses",
            Param::OpcionalEntero(metadatos.garantia_meses),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "es_materia_prima",
            Param::Booleano(metadatos.es_materia_prima),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "ubicacion",
            Param::Texto(metadatos.ubicacion),
        );
        agregar_set(
            &mut sets,
            &mut params,
            "estado",
            Param::Texto(metadatos.estado_operativo),
        );
        sets.push("actualizado_en = NOW()".to_string());

        if cambios.get("atributos").is_some() {
            let atributos = cambios.get("atributos").cloned().unwrap_or(Value::Null);
            params.push(Param::Json(atributos.to_string()));
            sets.push(format!("atributos = ${}::jsonb", params.len()));
        }

        if cambios.get("precio_base").is_some() {
            let nuevo = validar_precio(cambios.get("precio_base"))?;
            agregar_set(
                &mut sets,
                &mut params,
                "precio_cents",
                Param::Entero(nuevo.centavos),
            );
            let anterior = Money::parsear(&texto(&previo, "precio_base"))?.centavos;
            registrar_historico(
                &mut tx,
                &texto(&previo, "empresa_id"),
                &texto(&previo, "id"),
                Some(anterior),
                nuevo.centavos,
            )
            .await?;
        }

        if cambios.get("stock").is_some() {
            // El stock es de inventory-service (ADR-0018): item-service conserva
            // la columna como proyección de lectura, pero no la ajusta.
            return Err(ApiError::dominio(
                "STOCK_GESTIONADO_POR_INVENTARIO",
                "El stock se ajusta desde /api/v2/admin/inventario.",
            ));
        }

        params.push(Param::Texto(id.to_string()));
        if !sets.is_empty() {
            let sql = format!(
                "UPDATE item.items SET {} WHERE id = ${}::uuid",
                sets.join(", "),
                params.len()
            );
            let mut consulta = sqlx::query(&sql);
            for parametro in params {
                consulta = match parametro {
                    Param::Texto(valor) => consulta.bind(valor),
                    Param::Entero(valor) => consulta.bind(valor),
                    Param::OpcionalEntero(valor) => consulta.bind(valor),
                    Param::Booleano(valor) => consulta.bind(valor),
                    Param::Json(valor) => consulta.bind(valor),
                };
            }
            consulta.execute(&mut *tx).await.map_err(interno)?;
        }
        if let Some(padre_id) = previo.get("producto_padre_id").and_then(Value::as_str) {
            recalcular_padre(&mut tx, Uuid::parse_str(padre_id).map_err(interno)?).await?;
        }
        tx.commit().await.map_err(interno)?;

        self.encontrar_por_id(id, empresa_id, true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", id))
    }

    pub async fn eliminar(&self, id: &str, empresa_id: Option<&str>) -> Result<(), ApiError> {
        self.encontrar_por_id(id, empresa_id, true)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Producto", id))?;
        self.validar_sin_ofertas_activas(id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        match empresa_id {
            Some(empresa) => {
                sqlx::query(
                    "UPDATE item.items
                        SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
                      WHERE id = $1::uuid AND tipo = 'producto' AND eliminado_en IS NULL
                        AND empresa_id = $2::uuid",
                )
                .bind(id)
                .bind(empresa)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            }
            None => {
                sqlx::query(
                    "UPDATE item.items
                        SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
                      WHERE id = $1::uuid AND tipo = 'producto' AND eliminado_en IS NULL",
                )
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(interno)?;
            }
        }
        sqlx::query(
            "UPDATE item.items
                SET eliminado_en = NOW(), estado = 'inactivo', actualizado_en = NOW()
              WHERE parent_item_id = $1::uuid AND eliminado_en IS NULL",
        )
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    pub(super) async fn validar_codigo_barras(
        &self,
        empresa_id: &str,
        codigo_barras: &str,
        excluir_id: Option<&str>,
    ) -> Result<(), ApiError> {
        if codigo_barras.is_empty() {
            return Ok(());
        }
        let mut sql = "SELECT 1 FROM item.items
                        WHERE empresa_id = $1::uuid AND codigo_barras = $2 AND eliminado_en IS NULL"
            .to_string();
        if excluir_id.is_some() {
            sql.push_str(" AND id <> $3::uuid");
        }
        sql.push_str(" LIMIT 1");
        let mut consulta = sqlx::query(&sql).bind(empresa_id).bind(codigo_barras);
        if let Some(excluir) = excluir_id {
            consulta = consulta.bind(excluir);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = consulta.fetch_optional(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_some() {
            return Err(ApiError::conflicto(format!(
                "El código de barras {codigo_barras} ya está registrado."
            )));
        }
        Ok(())
    }

    pub(super) async fn sku_existe(&self, empresa_id: &str, sku: &str) -> Result<bool, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT 1 FROM item.items
              WHERE empresa_id = $1::uuid AND tipo = 'producto'
                AND LOWER(codigo) = LOWER($2) AND eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(sku)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(fila.is_some())
    }

    pub(super) async fn validar_sin_ofertas_activas(
        &self,
        producto_id: &str,
    ) -> Result<(), ApiError> {
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(4_000))
            .build()
            .map_err(|_| {
                ApiError::dominio(
                    "OFERTAS_NO_VERIFICABLES",
                    "No se pudo verificar si el producto tiene ofertas activas.",
                )
            })?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/ofertas/producto/{}/activas",
                self.stores_url,
                urlencoding::encode(producto_id)
            ))
            .header("x-internal-key", &self.clave_interna)
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio(
                    "OFERTAS_NO_VERIFICABLES",
                    "No se pudo verificar si el producto tiene ofertas activas.",
                )
            })?;
        if !respuesta.status().is_success() {
            return Err(ApiError::dominio(
                "OFERTAS_NO_VERIFICABLES",
                "No se pudo verificar si el producto tiene ofertas activas.",
            ));
        }
        let datos = respuesta.json::<Value>().await.map_err(|_| {
            ApiError::dominio(
                "OFERTAS_NO_VERIFICABLES",
                "No se pudo verificar si el producto tiene ofertas activas.",
            )
        })?;
        if datos.get("total").and_then(Value::as_i64).unwrap_or(0) > 0 {
            return Err(ApiError::dominio(
                "OFERTAS_ACTIVAS",
                "El producto tiene ofertas activas; desactívalas antes de cambiar sus variantes o imágenes.",
            ));
        }
        Ok(())
    }

    /// ADR-27: solo una empresa activa con la capacidad de catálogo tiene
    /// items. Falla cerrado si identity no responde (R-DB-1).
    pub async fn validar_empresa_catalogo(&self, empresa_id: &str) -> Result<(), ApiError> {
        if empresa_id.is_empty() || empresa_id == EMPRESA_LEGACY_ID {
            return Err(ApiError::dominio(
                "EMPRESA_NO_PUBLICA",
                "La empresa comodín del catálogo legado no publica productos.",
            ));
        }
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(self.timeout_identidad_ms))
            .build()
            .map_err(|_| {
                ApiError::dominio(
                    "EMPRESA_NO_VERIFICABLE",
                    "No se pudo verificar la empresa del catálogo.",
                )
            })?;
        let respuesta = cliente
            .get(format!(
                "{}/internal/empresas/{}",
                self.identidad_url,
                urlencoding::encode(empresa_id)
            ))
            .header("x-internal-key", &self.clave_interna)
            .send()
            .await
            .map_err(|_| {
                ApiError::dominio(
                    "EMPRESA_NO_VERIFICABLE",
                    "No se pudo verificar la empresa del catálogo.",
                )
            })?;
        if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(ApiError::no_encontrado("Empresa", empresa_id));
        }
        if !respuesta.status().is_success() {
            return Err(ApiError::dominio(
                "EMPRESA_NO_VERIFICABLE",
                "No se pudo verificar la empresa del catálogo.",
            ));
        }
        let empresa = respuesta.json::<Value>().await.map_err(|_| {
            ApiError::dominio(
                "EMPRESA_NO_VERIFICABLE",
                "No se pudo verificar la empresa del catálogo.",
            )
        })?;
        if empresa.get("estado").and_then(Value::as_str) != Some("activa") {
            return Err(ApiError::dominio(
                "EMPRESA_INACTIVA",
                "La empresa debe estar activa para operar su catálogo.",
            ));
        }
        let capacidades = self.capacidades_catalogo(empresa_id).await?;
        let tiene_capacidad = capacidades.iter().any(|capacidad| {
            capacidad.get("empresa_id").and_then(Value::as_str) == Some(empresa_id)
                && capacidad.get("capacidad_codigo").and_then(Value::as_str)
                    == Some("catalogo_items")
                && capacidad.get("activo").and_then(Value::as_bool) == Some(true)
        });
        if !tiene_capacidad {
            return Err(ApiError::dominio(
                "EMPRESA_SIN_CATALOGO",
                "La empresa no tiene activa la capacidad de catálogo.",
            ));
        }
        Ok(())
    }

    pub(super) async fn capacidades_catalogo(
        &self,
        empresa_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        let error = || {
            ApiError::dominio(
                "EMPRESA_NO_VERIFICABLE",
                "No se pudieron verificar las capacidades de la empresa.",
            )
        };
        let cliente = reqwest::Client::builder()
            .timeout(Duration::from_millis(self.timeout_identidad_ms))
            .build()
            .map_err(|_| error())?;
        let respuesta = cliente
            .post(format!(
                "{}/internal/empresas/capacidades",
                self.identidad_url
            ))
            .header("x-internal-key", &self.clave_interna)
            .json(&json!({
                "empresa_ids": [empresa_id],
                "capacidades": ["catalogo_items"],
            }))
            .send()
            .await
            .map_err(|_| error())?;
        if !respuesta.status().is_success() {
            return Err(error());
        }
        let datos = respuesta.json::<Value>().await.map_err(|_| error())?;
        Ok(datos
            .get("capacidades")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }
}
