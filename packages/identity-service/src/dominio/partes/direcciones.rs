use super::comunes::*;
use super::prelude::*;
use super::*;

impl Partes {
    pub async fn listar_direcciones(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, false)
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(d) AS datos FROM identity.direcciones_partes d
              WHERE d.empresa_id = $1::uuid AND d.parte_id = $2::uuid AND d.eliminado_en IS NULL
              ORDER BY d.es_principal DESC, d.tipo, d.id",
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
                    &["creado_en", "actualizado_en", "eliminado_en"],
                ))
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn crear_direccion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        let meta = normalizar_direccion(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.direcciones_partes AS d
               (empresa_id, parte_id, tipo, etiqueta, linea_1, linea_2, ciudad,
                departamento, pais, codigo_postal, referencias, latitud, longitud,
                es_principal, metadata)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8, $9, $10, $11,
                     $12, $13, $14, $15::jsonb)
             RETURNING to_jsonb(d) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(texto(&meta, "tipo"))
        .bind(texto(&meta, "etiqueta"))
        .bind(texto(&meta, "linea_1"))
        .bind(texto(&meta, "linea_2"))
        .bind(texto(&meta, "ciudad"))
        .bind(texto(&meta, "departamento"))
        .bind(texto(&meta, "pais"))
        .bind(texto(&meta, "codigo_postal"))
        .bind(texto(&meta, "referencias"))
        .bind(meta.get("latitud").and_then(Value::as_f64))
        .bind(meta.get("longitud").and_then(Value::as_f64))
        .bind(
            meta.get("es_principal")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        )
        .bind(meta.get("metadata").cloned().unwrap_or_else(|| json!({})))
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let fila = fila.ok_or_else(|| ApiError::conflicto("No se pudo crear la dirección."))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn actualizar_direccion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        direccion_id: &str,
        actor: &UsuarioContexto,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(direccion_id, "DIRECCION_INVALIDA")?;
        let meta = normalizar_direccion_parcial(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.direcciones_partes AS d
                SET tipo = COALESCE($4, tipo), etiqueta = COALESCE($5, etiqueta),
                    linea_1 = COALESCE($6, linea_1), linea_2 = COALESCE($7, linea_2),
                    ciudad = COALESCE($8, ciudad), departamento = COALESCE($9, departamento),
                    pais = COALESCE($10, pais), codigo_postal = COALESCE($11, codigo_postal),
                    referencias = COALESCE($12, referencias), latitud = COALESCE($13, latitud),
                    longitud = COALESCE($14, longitud), es_principal = COALESCE($15, es_principal),
                    metadata = COALESCE($16::jsonb, metadata), actualizado_en = NOW()
              WHERE d.empresa_id = $1::uuid AND d.parte_id = $2::uuid
                AND d.id = $3::uuid AND d.eliminado_en IS NULL
              RETURNING to_jsonb(d) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(direccion_id)
        .bind(meta.get("tipo").and_then(Value::as_str))
        .bind(meta.get("etiqueta").and_then(Value::as_str))
        .bind(meta.get("linea_1").and_then(Value::as_str))
        .bind(meta.get("linea_2").and_then(Value::as_str))
        .bind(meta.get("ciudad").and_then(Value::as_str))
        .bind(meta.get("departamento").and_then(Value::as_str))
        .bind(meta.get("pais").and_then(Value::as_str))
        .bind(meta.get("codigo_postal").and_then(Value::as_str))
        .bind(meta.get("referencias").and_then(Value::as_str))
        .bind(meta.get("latitud").and_then(Value::as_f64))
        .bind(meta.get("longitud").and_then(Value::as_f64))
        .bind(meta.get("es_principal").and_then(Value::as_bool))
        .bind(meta.get("metadata").cloned())
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Dirección", direccion_id))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(
            datos,
            &["creado_en", "actualizado_en", "eliminado_en"],
        ))
    }

    pub async fn eliminar_direccion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        direccion_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(direccion_id, "DIRECCION_INVALIDA")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.direcciones_partes
                SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND parte_id = $2::uuid
                AND id = $3::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(direccion_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Dirección", direccion_id))?;
        tx.commit().await.map_err(interno)?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        Ok(json!({ "ok": true }))
    }
}

pub(super) fn normalizar_direccion(datos: &Value) -> Result<Value, ApiError> {
    let parcial = normalizar_direccion_parcial(datos)?;
    if parcial.get("tipo").is_none() || parcial.get("linea_1").is_none() {
        return Err(ApiError::dominio(
            "DIRECCION_INVALIDA",
            "El tipo y la línea principal son obligatorios.",
        ));
    }
    let mut salida = parcial.as_object().cloned().unwrap_or_default();
    for clave in [
        "etiqueta",
        "linea_2",
        "ciudad",
        "departamento",
        "pais",
        "codigo_postal",
        "referencias",
    ] {
        if !salida.contains_key(clave) {
            salida.insert(clave.into(), json!(""));
        }
    }
    if !salida.contains_key("latitud") {
        salida.insert("latitud".into(), Value::Null);
    }
    if !salida.contains_key("longitud") {
        salida.insert("longitud".into(), Value::Null);
    }
    if !salida.contains_key("es_principal") {
        salida.insert("es_principal".into(), json!(false));
    }
    if !salida.contains_key("metadata") {
        salida.insert("metadata".into(), json!({}));
    }
    Ok(Value::Object(salida))
}

fn normalizar_direccion_parcial(datos: &Value) -> Result<Value, ApiError> {
    let Some(objeto) = datos.as_object() else {
        return Err(ApiError::dominio(
            "DIRECCION_INVALIDA",
            "La dirección debe ser un objeto JSON.",
        ));
    };
    let mut salida = Map::new();
    if let Some(tipo) = objeto.get("tipo") {
        let tipo = tipo.as_str().unwrap_or_default();
        if !TIPOS_DIRECCION.contains(&tipo) {
            return Err(ApiError::dominio(
                "TIPO_DIRECCION_INVALIDO",
                "El tipo de dirección no es válido.",
            ));
        }
        salida.insert("tipo".into(), json!(tipo));
    }
    for (clave, maximo) in [
        ("etiqueta", 120),
        ("linea_1", 240),
        ("linea_2", 240),
        ("ciudad", 120),
        ("departamento", 120),
        ("pais", 120),
        ("codigo_postal", 32),
        ("referencias", 500),
    ] {
        if let Some(valor) = objeto.get(clave) {
            salida.insert(clave.into(), json!(texto_max(valor, maximo)?));
        }
    }
    for (clave, minimo, maximo) in [("latitud", -90.0, 90.0), ("longitud", -180.0, 180.0)] {
        if let Some(valor) = objeto.get(clave) {
            if valor.is_null() {
                salida.insert(clave.into(), Value::Null);
            } else if let Some(numero) = valor.as_f64() {
                if numero < minimo || numero > maximo {
                    return Err(ApiError::dominio(
                        "COORDENADA_INVALIDA",
                        "La coordenada está fuera de rango.",
                    ));
                }
                salida.insert(clave.into(), json!(numero));
            } else {
                return Err(ApiError::dominio(
                    "COORDENADA_INVALIDA",
                    "La coordenada debe ser numérica.",
                ));
            }
        }
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
