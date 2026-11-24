use super::comunes::*;
use super::prelude::*;
use super::*;

impl Partes {
    pub async fn listar_relaciones(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<Value>, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, false)
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(r) || jsonb_build_object(
                      'relacionada_id', p.id,
                      'relacionada_nombre', p.nombre,
                      'relacionada_tipo', p.tipo
                    ) AS datos
               FROM identity.relaciones_partes r
               JOIN identity.partes p
                 ON p.id = CASE WHEN r.parte_origen_id = $2::uuid
                                THEN r.parte_destino_id ELSE r.parte_origen_id END
              WHERE r.empresa_id = $1::uuid
                AND (r.parte_origen_id = $2::uuid OR r.parte_destino_id = $2::uuid)
                AND r.activo = true AND p.eliminado_en IS NULL
              ORDER BY r.tipo, r.creado_en DESC",
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
                let mut datos: Value = fila.try_get("datos").map_err(interno)?;
                if let Some(objeto) = datos.as_object_mut() {
                    let relacionada = json!({
                        "id": objeto.remove("relacionada_id"),
                        "nombre": objeto.remove("relacionada_nombre"),
                        "tipo": objeto.remove("relacionada_tipo"),
                    });
                    objeto.insert("parte_relacionada".into(), relacionada);
                }
                Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
            })
            .collect()
    }

    pub async fn crear_relacion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        actor: &UsuarioContexto,
        destino_id: &str,
        tipo: &str,
        metadata: Option<&Value>,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(destino_id, "PARTE_DESTINO_INVALIDA")?;
        let tipo = normalizar_codigo(tipo, "TIPO_RELACION_INVALIDO")?;
        validar_metadata(metadata)?;
        if parte_id == destino_id {
            return Err(ApiError::dominio(
                "RELACION_INVALIDA",
                "Una parte no puede relacionarse consigo misma.",
            ));
        }
        self.obtener_parte(empresa_id, destino_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.relaciones_partes AS r
               (empresa_id, parte_origen_id, parte_destino_id, tipo, metadata, creado_por)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, COALESCE($5::jsonb, '{}'::jsonb), $6::uuid)
             RETURNING to_jsonb(r) AS datos",
        )
        .bind(empresa_id)
        .bind(parte_id)
        .bind(destino_id)
        .bind(&tipo)
        .bind(metadata.cloned())
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|error| {
            if es_unico(&error) {
                ApiError::conflicto("La relación ya existe.")
            } else {
                interno(error)
            }
        })?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear la relación."))?;
        tx.commit().await.map_err(interno)?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        Ok(normalizar_fechas(datos, &["creado_en", "actualizado_en"]))
    }

    pub async fn eliminar_relacion(
        &self,
        empresa_id: &str,
        parte_id: &str,
        relacion_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Value, ApiError> {
        self.validar_parte_acceso(empresa_id, parte_id, actor, true)
            .await?;
        validar_uuid(relacion_id, "RELACION_INVALIDA")?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE identity.relaciones_partes SET activo = false, actualizado_en = NOW()
              WHERE empresa_id = $1::uuid AND id = $2::uuid
                AND (parte_origen_id = $3::uuid OR parte_destino_id = $3::uuid) AND activo = true
              RETURNING id",
        )
        .bind(empresa_id)
        .bind(relacion_id)
        .bind(parte_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Relación", relacion_id))?;
        tx.commit().await.map_err(interno)?;
        let _: uuid::Uuid = fila.try_get("id").map_err(interno)?;
        Ok(json!({ "ok": true }))
    }
}

pub(super) fn normalizar_codigo(valor: &str, error: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    let mut chars = codigo.chars();
    let valido = matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&codigo.len())
        && codigo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valido {
        return Err(ApiError::dominio(
            error,
            "El código solo admite minúsculas, números y guion bajo (2-40).",
        ));
    }
    Ok(codigo)
}
