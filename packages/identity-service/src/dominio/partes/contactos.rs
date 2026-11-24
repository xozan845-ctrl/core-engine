use super::comunes::*;
use super::prelude::*;
use super::*;

impl Partes {
    pub async fn listar_contactos(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, false)
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(c) AS datos FROM identity.medios_contacto c
              WHERE c.empresa_id = $1::uuid AND c.parte_id = $2::uuid AND c.eliminado_en IS NULL
              ORDER BY c.es_principal DESC, c.tipo, c.valor",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(
                    datos,
                    &[
                        "verificado_en",
                        "creado_en",
                        "actualizado_en",
                        "eliminado_en",
                    ],
                ))
            })
            .collect()
    }

    pub async fn crear_contacto(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        let meta = normalizar_contacto(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.medios_contacto AS c
               (empresa_id, parte_id, tipo, valor, etiqueta, es_principal, metadata)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7::jsonb)
             RETURNING to_jsonb(c) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(texto(&meta, "tipo"))
        .bind(texto(&meta, "valor"))
        .bind(texto(&meta, "etiqueta"))
        .bind(
            meta.get("es_principal")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .bind(meta.get("metadata").cloned().unwrap_or_else(|| json!({})))
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_contacto)?;
        tx.commit().await.map_err(interno)?;
        let fila =
            fila.ok_or_else(|| ApiError::conflicto("No se pudo crear el medio de contacto."))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &[
                "verificado_en",
                "creado_en",
                "actualizado_en",
                "eliminado_en",
            ],
        ))
    }

    pub async fn actualizar_contacto(
        &self,
        empresa_id: &str,
        parte_id: &str,
        contacto_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(contacto_id, "CONTACTO_INVALIDO")?;
        let meta = normalizar_contacto_parcial(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.medios_contacto AS c
                SET tipo = COALESCE($4, tipo), valor = COALESCE($5, valor),
                    etiqueta = COALESCE($6, etiqueta), es_principal = COALESCE($7, es_principal),
                    metadata = COALESCE($8::jsonb, metadata), actualizado_en = NOW()
              WHERE c.empresa_id = $1::uuid AND c.parte_id = $2::uuid
                AND c.id = $3::uuid AND c.eliminado_en IS NULL
              RETURNING to_jsonb(c) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(contacto_id)
        .bind(meta.get("tipo").and_then(Value::as_str))
        .bind(meta.get("valor").and_then(Value::as_str))
        .bind(meta.get("etiqueta").and_then(Value::as_str))
        .bind(meta.get("es_principal").and_then(Value::as_bool))
        .bind(meta.get("metadata").cloned())
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_contacto)?
        .ok_or_else(|| ApiError::no_encontrado("Medio de contacto", contacto_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &[
                "verificado_en",
                "creado_en",
                "actualizado_en",
                "eliminado_en",
            ],
        ))
    }

    pub async fn eliminar_contacto(
        &self,
        empresa_id: &str,
        parte_id: &str,
        contacto_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(contacto_id, "CONTACTO_INVALIDO")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.medios_contacto
                SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid
                AND id = $3::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(contacto_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Medio de contacto", contacto_id))?;
        tx.commit().await.map_err(interno)?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        Ok(json!({ "ok": true }))
    }
}

fn normalizar_contacto(datos: &Value) -> Result<Value, ApiError> {
    let parcial = normalizar_contacto_parcial(datos)?;
    if parcial.get("tipo").is_none() || parcial.get("valor").is_none() {
        return Err(ApiError::dominio(
            "CONTACTO_INVALIDO",
            "El tipo y valor del contacto son obligatorios.",
        ));
    }
    let mut salida = parcial.as_object().cloned().unwrap_or_default();
    salida.insert("etiqueta".into(), json!(texto(&parcial, "etiqueta")));
    salida.insert(
        "es_principal".into(),
        parcial
            .get("es_principal")
            .cloned()
            .unwrap_or_else(|| json!(false)),
    );
    salida.insert(
        "metadata".into(),
        parcial
            .get("metadata")
            .cloned()
            .unwrap_or_else(|| json!({})),
    );
    Ok(Value::Object(salida))
}

fn normalizar_contacto_parcial(datos: &Value) -> Result<Value, ApiError> {
    let Some(objeto) = datos.as_object() else {
        return Err(ApiError::dominio(
            "CONTACTO_INVALIDO",
            "El contacto debe ser un objeto JSON.",
        ));
    };
    let mut salida = Map::new();
    if let Some(tipo) = objeto.get("tipo") {
        let tipo = tipo.as_str().unwrap_or_default();
        if !TIPOS_CONTACTO.contains(&tipo) {
            return Err(ApiError::dominio(
                "TIPO_CONTACTO_INVALIDO",
                "El tipo de contacto no es válido.",
            ));
        }
        salida.insert("tipo".into(), json!(tipo));
    }
    if let Some(valor) = objeto.get("valor") {
        salida.insert("valor".into(), json!(texto_max(valor, 240)?));
    }
    if let Some(etiqueta) = objeto.get("etiqueta") {
        salida.insert("etiqueta".into(), json!(texto_max(etiqueta, 120)?));
    }
    if let Some(es_principal) = objeto.get("es_principal") {
        salida.insert("es_principal".into(), es_principal.clone());
    }
    if let Some(metadata) = objeto.get("metadata") {
        validar_metadata(Some(metadata))?;
        salida.insert("metadata".into(), metadata.clone());
    }
    Ok(Value::Object(salida))
}

fn conflicto_contacto(error: sqlx::Error) -> ApiError {
    if es_unico(&error) {
        return ApiError::conflicto("Ya existe ese medio de contacto para la parte.");
    }
    interno(error)
}
