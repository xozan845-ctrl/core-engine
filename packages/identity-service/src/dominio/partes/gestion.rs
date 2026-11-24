use super::comunes::*;
use super::prelude::*;
use super::*;

impl Partes {
    pub async fn listar(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
        q: Option<&str>,
        tipo: Option<&str>,
        limite: Option<i64>,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_acceso(empresa_id, actor, false).await?;
        let mut condiciones = vec![
            "p.empresa_id = $1::uuid".to_string(),
            "p.eliminado_en IS NULL".to_string(),
        ];
        let mut parametros: Vec<String> = vec![empresa_id.to_string()];
        if let Some(tipo) = tipo {
            if !TIPOS_PARTE.contains(&tipo) {
                return Err(ApiError::dominio(
                    "TIPO_PARTE_INVALIDO",
                    "El tipo de parte no es válido.",
                ));
            }
            parametros.push(tipo.to_string());
            condiciones.push(format!("p.tipo = ${}", parametros.len()));
        }
        if let Some(q) = q.map(str::trim).filter(|valor| !valor.is_empty()) {
            parametros.push(format!("%{q}%"));
            condiciones.push(format!(
                "(p.nombre ILIKE ${0} OR p.nombre_comercial ILIKE ${0} OR p.identificador_valor ILIKE ${0})",
                parametros.len()
            ));
        }
        let limite = limite.unwrap_or(50).clamp(1, 100);
        parametros.push(limite.to_string());
        let sql = format!(
            "SELECT to_jsonb(p) AS datos FROM identity.partes p
              WHERE {}
              ORDER BY p.actualizado_en DESC, p.id DESC
              LIMIT ${}::int",
            condiciones.join(" AND "),
            parametros.len()
        );
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let mut consulta = sqlx::query(&sql);
        for parametro in &parametros {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta.fetch_all(&mut *tx).await.map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let datos: Value = fila.try_get("datos").map_err(interno)?;
                Ok(normalizar_fechas(
                    datos,
                    &["creado_en", "actualizado_en", "eliminado_en"],
                ))
            })
            .collect()
    }

    pub async fn obtener(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, actor, false).await?;
        self.obtener_parte(empresa_id, parte_id).await
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_acceso(empresa_id, actor, true).await?;
        let meta = normalizar_parte(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.partes AS p
               (empresa_id, tipo, nombre, nombre_comercial, identificador_tipo,
                identificador_valor, estado, metadata, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8::jsonb, $9::uuid)
             RETURNING to_jsonb(p) AS datos",
        )
        .bind(empresa_id)
        .bind(texto(&meta, "tipo"))
        .bind(texto(&meta, "nombre"))
        .bind(texto(&meta, "nombre_comercial"))
        .bind(texto(&meta, "identificador_tipo"))
        .bind(texto(&meta, "identificador_valor"))
        .bind(texto(&meta, "estado"))
        .bind(meta.get("metadata").cloned().unwrap_or_else(|| json!({})))
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_identificador)?;
        tx.commit().await.map_err(interno)?;
        let fila = fila.ok_or_else(|| ApiError::conflicto("No se pudo crear la parte."))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    pub async fn actualizar(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        let meta = normalizar_parte_parcial(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.partes AS p
                SET tipo = COALESCE($3, tipo),
                    nombre = COALESCE($4, nombre),
                    nombre_comercial = COALESCE($5, nombre_comercial),
                    identificador_tipo = COALESCE($6, identificador_tipo),
                    identificador_valor = COALESCE($7, identificador_valor),
                    estado = COALESCE($8, estado),
                    metadata = COALESCE($9::jsonb, metadata),
                    actualizado_en = NOW()
              WHERE p.empresa_id = $1::uuid AND p.id = $2::uuid AND p.eliminado_en IS NULL
              RETURNING to_jsonb(p) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(meta.get("tipo").and_then(Value::as_str))
        .bind(meta.get("nombre").and_then(Value::as_str))
        .bind(meta.get("nombre_comercial").and_then(Value::as_str))
        .bind(meta.get("identificador_tipo").and_then(Value::as_str))
        .bind(meta.get("identificador_valor").and_then(Value::as_str))
        .bind(meta.get("estado").and_then(Value::as_str))
        .bind(meta.get("metadata").cloned())
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_identificador)?
        .ok_or_else(|| ApiError::no_encontrado("Parte", parte_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    pub async fn eliminar(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.partes
                SET estado = 'archivado', eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Parte", parte_id))?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        sqlx::query(
            "UPDATE identity.medios_contacto SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.direcciones_partes SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE identity.relaciones_partes SET activo = false, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid
                AND (parte_origen_id = $2::uuid OR parte_destino_id = $2::uuid) AND activo = true",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    /// Validación interna (servicio→servicio): ¿la parte vive en esa empresa?
    pub async fn pertenece_a_empresa(
        &self,
        empresa_id: &str,
        parte_id: &str,
    ) -> Result<bool, ApiError> {
        if !es_uuid(empresa_id) || !es_uuid(parte_id) {
            return Ok(false);
        }
        let mut tx = self.db.transaccion_servicio().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT 1 FROM identity.partes
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(parte_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(fila.is_some())
    }

    pub(super) async fn obtener_parte(
        &self,
        empresa_id: &str,
        parte_id: &str,
    ) -> Result<Value, ApiError> {
        validar_uuid(parte_id, "PARTE_INVALIDA")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(p) AS datos FROM identity.partes p
              WHERE p.empresa_id = $1::uuid AND p.id = $2::uuid AND p.eliminado_en IS NULL",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Parte", parte_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    pub(super) async fn validar_parte_acceso(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        escritura: bool,
    ) -> Result<(), ApiError> {
        self.validar_acceso(empresa_id, actor, escritura).await?;
        self.obtener_parte(empresa_id, parte_id).await?;
        Ok(())
    }

    pub(super) async fn validar_acceso(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
        escritura: bool,
    ) -> Result<(), ApiError> {
        validar_uuid(empresa_id, "EMPRESA_INVALIDA")?;
        if actor.es_admin_global() {
            return Ok(());
        }
        if actor.tenant_id.as_deref() != Some(empresa_id) {
            return Err(ApiError::acceso_denegado(
                "No puedes operar partes de otra empresa.",
            ));
        }
        if escritura && !ROLES_ESCRITURA.contains(&actor.rol.as_str()) {
            return Err(ApiError::acceso_denegado(
                "El rol no puede modificar los datos universales.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let miembro = sqlx::query(
            "SELECT 1 FROM identity.asignaciones_personal
              WHERE empresa_id = $1::uuid AND usuario_id = $2::uuid AND estado = 'activa'",
        )
        .bind(empresa_id)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if miembro.is_none() {
            return Err(ApiError::acceso_denegado(
                "El usuario no es miembro activo de esta empresa.",
            ));
        }
        Ok(())
    }
}

pub(super) fn normalizar_parte(datos: &Value) -> Result<Value, ApiError> {
    let parcial = normalizar_parte_parcial(datos)?;
    let tipo = parcial.get("tipo").and_then(Value::as_str);
    let nombre = parcial.get("nombre").and_then(Value::as_str);
    if tipo.is_none() || nombre.is_none() {
        return Err(ApiError::dominio(
            "PARTE_INVALIDA",
            "El tipo y nombre de la parte son obligatorios.",
        ));
    }
    let mut salida = parcial.as_object().cloned().unwrap_or_default();
    salida.insert(
        "nombre_comercial".into(),
        json!(texto(&parcial, "nombre_comercial")),
    );
    salida.insert(
        "identificador_tipo".into(),
        json!(texto(&parcial, "identificador_tipo")),
    );
    salida.insert(
        "identificador_valor".into(),
        json!(texto(&parcial, "identificador_valor")),
    );
    salida.insert(
        "estado".into(),
        parcial
            .get("estado")
            .cloned()
            .unwrap_or_else(|| json!("activo")),
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

pub(super) fn normalizar_parte_parcial(datos: &Value) -> Result<Value, ApiError> {
    let Some(objeto) = datos.as_object() else {
        return Err(ApiError::dominio(
            "PARTE_INVALIDA",
            "La parte debe ser un objeto JSON.",
        ));
    };
    let mut salida = Map::new();
    if let Some(tipo) = objeto.get("tipo") {
        let tipo = tipo.as_str().unwrap_or_default();
        if !TIPOS_PARTE.contains(&tipo) {
            return Err(ApiError::dominio(
                "TIPO_PARTE_INVALIDO",
                "El tipo de parte no es válido.",
            ));
        }
        salida.insert("tipo".into(), json!(tipo));
    }
    if let Some(estado) = objeto.get("estado") {
        let estado = estado.as_str().unwrap_or_default();
        if !ESTADOS_PARTE.contains(&estado) {
            return Err(ApiError::dominio(
                "ESTADO_PARTE_INVALIDO",
                "El estado de parte no es válido.",
            ));
        }
        salida.insert("estado".into(), json!(estado));
    }
    if let Some(nombre) = objeto.get("nombre") {
        let nombre = nombre.as_str().unwrap_or("").trim();
        if nombre.is_empty() || nombre.chars().count() > 240 {
            return Err(ApiError::dominio(
                "NOMBRE_PARTE_INVALIDO",
                "El nombre debe tener entre 1 y 240 caracteres.",
            ));
        }
        salida.insert("nombre".into(), json!(nombre));
    }
    for (clave, maximo) in [
        ("nombre_comercial", 240),
        ("identificador_tipo", 40),
        ("identificador_valor", 120),
    ] {
        if let Some(valor) = objeto.get(clave) {
            salida.insert(clave.into(), json!(texto_max(valor, maximo)?));
        }
    }
    if let Some(metadata) = objeto.get("metadata") {
        validar_metadata(Some(metadata))?;
        salida.insert("metadata".into(), metadata.clone());
    }
    Ok(Value::Object(salida))
}

fn conflicto_identificador(error: sqlx::Error) -> ApiError {
    if es_unico(&error) {
        return ApiError::conflicto("Ya existe una parte con ese identificador.");
    }
    interno(error)
}
