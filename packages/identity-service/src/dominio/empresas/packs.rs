use super::comunes::*;
use super::prelude::*;
use super::*;

impl Empresas {
    pub async fn actualizar_pack(
        &self,
        empresa_id: &str,
        pack_codigo: &str,
        version: Option<i32>,
    ) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT to_jsonb(e) AS datos FROM identity.empresas e
              WHERE e.id = $1::uuid FOR UPDATE",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", empresa_id))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        let vertical = datos.get("vertical_id").and_then(Value::as_str);
        let pack = resolver_pack(&mut tx, pack_codigo, vertical, version).await?;
        if texto(&datos, "pack_codigo") == pack.codigo
            && datos.get("pack_version").and_then(Value::as_i64) == Some(i64::from(pack.version))
        {
            tx.commit().await.map_err(interno)?;
            return Ok(Self::con_urls(&datos));
        }
        // El cambio de pack no puede dejar al tenant en una región que el
        // nuevo pack no admite: sería una empresa fiscalmente inválida.
        let region_actual = texto(&datos, "region_tributaria");
        if !region_actual.is_empty() {
            let soportada = sqlx::query(
                "SELECT 1 FROM identity.pack_regiones
                  WHERE pack_codigo = $1 AND pack_version = $2
                    AND region_codigo = $3 AND activo = true",
            )
            .bind(&pack.codigo)
            .bind(pack.version)
            .bind(&region_actual)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
            if soportada.is_none() {
                return Err(ApiError::dominio(
                    "REGION_NO_COMPATIBLE_CON_PACK",
                    "El pack destino no admite la región tributaria actual de la empresa.",
                )
                .con_detalles(json!({
                    "region_actual": region_actual,
                    "pack_codigo": pack.codigo,
                    "pack_version": pack.version,
                })));
            }
        }
        let antes: i32 = sqlx::query_scalar(
            "SELECT COUNT(*)::int FROM identity.empresa_capacidades WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        let capacidades = sqlx::query(
            "SELECT capacidad_codigo, config, activo
               FROM identity.pack_capacidades
              WHERE pack_codigo = $1 AND pack_version = $2",
        )
        .bind(&pack.codigo)
        .bind(pack.version)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        let codigos: Vec<String> = capacidades
            .iter()
            .map(|fila| fila.try_get("capacidad_codigo").map_err(interno))
            .collect::<Result<_, _>>()?;
        sqlx::query(
            "DELETE FROM identity.empresa_capacidades
              WHERE empresa_id = $1::uuid AND NOT (capacidad_codigo = ANY($2::text[]))",
        )
        .bind(empresa_id)
        .bind(&codigos)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        for capacidad in &capacidades {
            sqlx::query(
                "INSERT INTO identity.empresa_capacidades AS ec
                   (empresa_id, capacidad_codigo, config, activo)
                 VALUES ($1::uuid, $2, $3::jsonb, $4)
                 ON CONFLICT (empresa_id, capacidad_codigo) DO UPDATE
                   SET config = EXCLUDED.config,
                       activo = EXCLUDED.activo,
                       actualizado_en = NOW()",
            )
            .bind(empresa_id)
            .bind(
                capacidad
                    .try_get::<String, _>("capacidad_codigo")
                    .map_err(interno)?,
            )
            .bind(capacidad.try_get::<Value, _>("config").map_err(interno)?)
            .bind(capacidad.try_get::<bool, _>("activo").map_err(interno)?)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        }
        let actualizada = sqlx::query(
            "UPDATE identity.empresas AS e
                SET pack_codigo = $2, pack_version = $3
              WHERE e.id = $1::uuid
              RETURNING to_jsonb(e) AS datos",
        )
        .bind(empresa_id)
        .bind(&pack.codigo)
        .bind(pack.version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", empresa_id))?;
        let datos_nuevos: Value = actualizada.try_get("datos").map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.pack_migraciones
               (empresa_id, vertical_origen, pack_codigo, pack_version,
                capacidades_antes, capacidades_despues, estado, detalles)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, 'aplicado', $7::jsonb)
             ON CONFLICT (empresa_id) DO UPDATE
               SET pack_codigo = EXCLUDED.pack_codigo,
                   pack_version = EXCLUDED.pack_version,
                   capacidades_antes = EXCLUDED.capacidades_antes,
                   capacidades_despues = EXCLUDED.capacidades_despues,
                   estado = EXCLUDED.estado,
                   detalles = EXCLUDED.detalles,
                   migrado_en = NOW()",
        )
        .bind(empresa_id)
        .bind(vertical)
        .bind(&pack.codigo)
        .bind(pack.version)
        .bind(antes)
        .bind(capacidades.len() as i32)
        .bind(json!({
            "origen": "pack_upgrade",
            "pack_anterior": datos.get("pack_codigo"),
            "version_anterior": datos.get("pack_version"),
            "pack_nuevo": pack.codigo,
            "version_nueva": pack.version,
        }))
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(Self::con_urls(&datos_nuevos))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PackResuelto {
    pub(super) codigo: String,
    pub(super) version: i32,
    pub(super) vertical_codigo: String,
}

pub(super) async fn resolver_pack(
    tx: &mut Transaction<'_, Postgres>,
    codigo: &str,
    vertical: Option<&str>,
    version: Option<i32>,
) -> Result<PackResuelto, ApiError> {
    let fila = match version {
        Some(version) => sqlx::query(
            "SELECT codigo, version, vertical_codigo FROM identity.packs
              WHERE codigo = $1 AND version = $2 AND activo = true",
        )
        .bind(codigo)
        .bind(version)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?,
        None => sqlx::query(
            "SELECT codigo, version, vertical_codigo FROM identity.packs
              WHERE codigo = $1 AND activo = true
              ORDER BY version DESC LIMIT 1",
        )
        .bind(codigo)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?,
    };
    let Some(fila) = fila else {
        return Err(ApiError::dominio(
            "PACK_INVALIDO",
            format!("El pack \"{codigo}\" no existe o esta inactivo."),
        ));
    };
    let pack = PackResuelto {
        codigo: fila.try_get("codigo").map_err(interno)?,
        version: fila.try_get::<i32, _>("version").map_err(interno)?,
        vertical_codigo: fila.try_get("vertical_codigo").map_err(interno)?,
    };
    if let Some(vertical) = vertical {
        if vertical != pack.vertical_codigo {
            return Err(ApiError::dominio(
                "PACK_VERTICAL_INCOMPATIBLE",
                "El pack no corresponde a la vertical solicitada.",
            ));
        }
    }
    Ok(pack)
}
