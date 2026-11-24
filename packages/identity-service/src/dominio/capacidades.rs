//! Capacidades por empresa (R-AR-5).
//!
//! Réplica de `capacidades/capacidades.service.ts`: el admin global gestiona
//! cualquier empresa; el empresario solo su tenant y solo si sigue siendo
//! miembro activo con rol `empresario`.

use crate::auth::UsuarioContexto;
use crate::db::{iso_ms, Db};
use crate::dominio::auditoria;
use crate::dominio::features::features_de;
use crate::dominio::regiones::{region_de_empresa_en, Regiones};
use crate::errors::ApiError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EmpresaCapacidad {
    pub capacidad_codigo: String,
    pub nombre: String,
    pub modulo: String,
    pub activo: bool,
    pub config: Value,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PackManifiesto {
    pub codigo: String,
    pub version: i32,
    pub nombre: String,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModeloManifiesto {
    pub codigo: String,
    pub nombre: String,
    pub descripcion: String,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CapacidadManifiesto {
    pub codigo: String,
    pub nombre: String,
    pub modulo: String,
    pub activo: bool,
    pub config: Value,
    pub features: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RegionManifiesto {
    pub codigo: String,
    pub nombre: String,
    pub pais_iso: String,
    pub moneda: String,
    pub moneda_iso: String,
    pub simbolo_moneda: String,
    pub locale: String,
    pub zona_horaria: String,
    pub tasa_iva: f64,
    pub documento_fiscal: Value,
    /// Features fiscales de la región, solo si `facturacion` está activa.
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Manifiesto {
    pub empresa_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negocio_id: Option<String>,
    pub vertical_id: Option<String>,
    pub pack: Option<PackManifiesto>,
    /// Región tributaria elegida por el tenant y features fiscales derivadas.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_tributaria: Option<RegionManifiesto>,
    pub modelos: Vec<ModeloManifiesto>,
    pub capacidades: Vec<CapacidadManifiesto>,
    pub features: Vec<String>,
}

pub struct Capacidades {
    db: Arc<Db>,
}

impl Capacidades {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<EmpresaCapacidad>, ApiError> {
        validar_empresa(empresa_id)?;
        self.validar_acceso(empresa_id, actor).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT c.codigo AS capacidad_codigo, c.nombre, c.modulo,
                    ec.activo, ec.config, ec.actualizado_en
               FROM identity.empresa_capacidades ec
               JOIN identity.capacidades c ON c.codigo = ec.capacidad_codigo
              WHERE ec.empresa_id = $1::uuid
              ORDER BY c.modulo, c.codigo",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas.iter().map(fila_a_capacidad).collect()
    }

    /// Manifiesto efectivo: vertical, pack, modelos, capacidades y features.
    pub async fn manifiesto(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Manifiesto, ApiError> {
        validar_empresa(empresa_id)?;
        self.validar_acceso(empresa_id, actor).await?;
        self.manifiesto_de(empresa_id, None).await
    }

    /// Manifiesto efectivo de un negocio. Si el negocio no tiene capacidades
    /// propias, hereda las de la empresa (mismo fallback que el guard interno).
    pub async fn manifiesto_negocio(
        &self,
        empresa_id: &str,
        negocio_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Manifiesto, ApiError> {
        validar_empresa(empresa_id)?;
        validar_negocio(negocio_id)?;
        self.validar_acceso_negocio(empresa_id, negocio_id, actor, "capacidades.leer")
            .await?;
        self.manifiesto_de(empresa_id, Some(negocio_id)).await
    }

    async fn manifiesto_de(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Manifiesto, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;

        let empresa = sqlx::query(
            "SELECT vertical_id, pack_codigo, pack_version
               FROM identity.empresas WHERE id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Empresa", empresa_id))?;
        let vertical_id: Option<String> = empresa.try_get("vertical_id").map_err(interno)?;
        let pack_codigo: Option<String> = empresa.try_get("pack_codigo").map_err(interno)?;
        let pack_version: Option<i32> = empresa.try_get("pack_version").map_err(interno)?;

        let mut pack = None;
        let mut modelos = Vec::new();
        if let (Some(codigo), Some(version)) = (pack_codigo.as_deref(), pack_version) {
            let fila = sqlx::query(
                "SELECT nombre, config FROM identity.packs
                  WHERE codigo = $1 AND version = $2",
            )
            .bind(codigo)
            .bind(version)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
            if let Some(fila) = fila {
                pack = Some(PackManifiesto {
                    codigo: codigo.to_string(),
                    version,
                    nombre: fila.try_get("nombre").map_err(interno)?,
                    config: fila.try_get("config").map_err(interno)?,
                });
            }
            let filas = sqlx::query(
                "SELECT mo.codigo, mo.nombre, mo.descripcion, mo.config
                   FROM identity.pack_modelos pm
                   JOIN identity.modelos_operacion mo ON mo.codigo = pm.modelo_codigo
                  WHERE pm.pack_codigo = $1 AND pm.pack_version = $2
                    AND pm.activo = true AND mo.activo = true
                  ORDER BY mo.codigo",
            )
            .bind(codigo)
            .bind(version)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
            for fila in &filas {
                modelos.push(ModeloManifiesto {
                    codigo: fila.try_get("codigo").map_err(interno)?,
                    nombre: fila.try_get("nombre").map_err(interno)?,
                    descripcion: fila.try_get("descripcion").map_err(interno)?,
                    config: fila.try_get("config").map_err(interno)?,
                });
            }
        }

        let filas = match negocio_id {
            Some(negocio_id) => {
                let propias = sqlx::query(
                    "SELECT c.codigo, c.nombre, c.modulo, nc.activo, nc.config
                       FROM identity.negocio_capacidades nc
                       JOIN identity.capacidades c ON c.codigo = nc.capacidad_codigo
                      WHERE nc.negocio_id = $1::uuid
                      ORDER BY c.modulo, c.codigo",
                )
                .bind(negocio_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(interno)?;
                if propias.is_empty() {
                    capacidades_de_empresa(&mut tx, empresa_id).await?
                } else {
                    propias
                }
            }
            None => capacidades_de_empresa(&mut tx, empresa_id).await?,
        };
        let region = region_de_empresa_en(&mut tx, empresa_id).await?;
        tx.commit().await.map_err(interno)?;

        let mut capacidades = Vec::with_capacity(filas.len());
        for fila in &filas {
            let codigo: String = fila.try_get("codigo").map_err(interno)?;
            capacidades.push(CapacidadManifiesto {
                features: features_de(&codigo).to_vec(),
                codigo,
                nombre: fila.try_get("nombre").map_err(interno)?,
                modulo: fila.try_get("modulo").map_err(interno)?,
                activo: fila.try_get("activo").map_err(interno)?,
                config: fila.try_get("config").map_err(interno)?,
            });
        }
        let facturacion_activa = capacidades
            .iter()
            .any(|capacidad| capacidad.codigo == "facturacion" && capacidad.activo);
        let region_tributaria = region.map(|region| RegionManifiesto {
            codigo: region.region.codigo.clone(),
            nombre: region.region.nombre.clone(),
            pais_iso: region.region.pais_iso.clone(),
            moneda: region.region.moneda.clone(),
            moneda_iso: region.region.moneda_iso.clone(),
            simbolo_moneda: region.region.simbolo_moneda.clone(),
            locale: region.region.locale.clone(),
            zona_horaria: region.region.zona_horaria.clone(),
            tasa_iva: region.region.tasa_iva,
            documento_fiscal: region.region.documento_fiscal.clone(),
            features: Regiones::features_fiscales(&region.region, facturacion_activa),
        });

        let mut features: Vec<String> = capacidades
            .iter()
            .filter(|capacidad| capacidad.activo)
            .flat_map(|capacidad| capacidad.features.iter().copied())
            .map(str::to_string)
            .collect();
        if let Some(region) = &region_tributaria {
            features.extend(region.features.iter().cloned());
        }
        features.sort();
        features.dedup();

        Ok(Manifiesto {
            empresa_id: empresa_id.to_string(),
            negocio_id: negocio_id.map(str::to_string),
            vertical_id,
            pack,
            region_tributaria,
            modelos,
            capacidades,
            features,
        })
    }

    pub async fn actualizar(
        &self,
        empresa_id: &str,
        capacidad_codigo: &str,
        activo: bool,
        config: Option<Value>,
        actor: &UsuarioContexto,
    ) -> Result<EmpresaCapacidad, ApiError> {
        validar_empresa(empresa_id)?;
        self.validar_acceso(empresa_id, actor).await?;
        let codigo = capacidad_codigo.trim().to_lowercase();
        if codigo.is_empty() {
            return Err(ApiError::dominio(
                "CAPACIDAD_INVALIDA",
                "La capacidad es obligatoria.",
            ));
        }
        let config_json = config;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let existe = sqlx::query(
            "SELECT c.codigo
               FROM identity.capacidades c
               JOIN identity.empresas e ON e.id = $1::uuid
               JOIN identity.vertical_capacidades vc
                 ON vc.vertical_codigo = e.vertical_id
                AND vc.capacidad_codigo = c.codigo
              WHERE c.codigo = $2 AND c.activo = true",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if existe.is_none() {
            return Err(ApiError::conflicto(format!(
                "La capacidad \"{codigo}\" no existe o esta inactiva."
            )));
        }
        let fila = sqlx::query(
            "INSERT INTO identity.empresa_capacidades AS ec
               (empresa_id, capacidad_codigo, activo, config, actualizado_en)
             VALUES ($1::uuid, $2, $3, COALESCE($4::jsonb, '{}'::jsonb), NOW())
             ON CONFLICT (empresa_id, capacidad_codigo) DO UPDATE
               SET activo = EXCLUDED.activo,
                   config = COALESCE($4::jsonb, ec.config),
                   actualizado_en = NOW()
             RETURNING capacidad_codigo, activo, config, actualizado_en,
               (SELECT nombre FROM identity.capacidades WHERE codigo = $2) AS nombre,
               (SELECT modulo FROM identity.capacidades WHERE codigo = $2) AS modulo",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .bind(activo)
        .bind(config_json.clone())
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo actualizar la capacidad."))?;
        // El cambio de empresa se replica en todos sus negocios para que el
        // guard por negocio y el contrato legado no diverjan.
        sqlx::query(
            "INSERT INTO identity.negocio_capacidades AS nc
               (negocio_id, capacidad_codigo, activo, config, actualizado_en)
             SELECT n.id, $2, $3, COALESCE($4::jsonb, '{}'::jsonb), NOW()
               FROM identity.negocios n
              WHERE n.empresa_id = $1::uuid
             ON CONFLICT (negocio_id, capacidad_codigo) DO UPDATE
               SET activo = EXCLUDED.activo,
                   config = COALESCE($4::jsonb, nc.config),
                   actualizado_en = NOW()",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .bind(activo)
        .bind(config_json)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            None,
            "capacidad.actualizada",
            "capacidad",
            Some(&codigo),
            serde_json::json!({ "activo": activo }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        fila_a_capacidad(&fila)
    }

    async fn validar_acceso(
        &self,
        empresa_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<(), ApiError> {
        if actor.es_admin_global() {
            return Ok(());
        }
        if actor.rol != "empresario" || actor.tenant_id.as_deref() != Some(empresa_id) {
            return Err(ApiError::acceso_denegado(
                "Solo el empresario activo de la empresa puede gestionar sus capacidades.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let miembro = sqlx::query(
            "SELECT 1
               FROM identity.asignaciones_personal a
               JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
              WHERE a.empresa_id = $1::uuid AND a.usuario_id = $2::uuid
                AND r.codigo = 'empresario' AND a.estado = 'activa'",
        )
        .bind(empresa_id)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if miembro.is_none() {
            return Err(ApiError::acceso_denegado(
                "El usuario no es empresario activo de esta empresa.",
            ));
        }
        Ok(())
    }
}

async fn capacidades_de_empresa(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa_id: &str,
) -> Result<Vec<sqlx::postgres::PgRow>, ApiError> {
    sqlx::query(
        "SELECT c.codigo, c.nombre, c.modulo, ec.activo, ec.config
           FROM identity.empresa_capacidades ec
           JOIN identity.capacidades c ON c.codigo = ec.capacidad_codigo
          WHERE ec.empresa_id = $1::uuid
          ORDER BY c.modulo, c.codigo",
    )
    .bind(empresa_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(interno)
}

fn fila_a_capacidad(fila: &sqlx::postgres::PgRow) -> Result<EmpresaCapacidad, ApiError> {
    let actualizado_en: DateTime<Utc> = fila.try_get("actualizado_en").map_err(interno)?;
    Ok(EmpresaCapacidad {
        capacidad_codigo: fila.try_get("capacidad_codigo").map_err(interno)?,
        nombre: fila.try_get("nombre").map_err(interno)?,
        modulo: fila.try_get("modulo").map_err(interno)?,
        activo: fila.try_get("activo").map_err(interno)?,
        config: fila.try_get("config").map_err(interno)?,
        actualizado_en: iso_ms(actualizado_en),
    })
}

fn validar_empresa(empresa_id: &str) -> Result<(), ApiError> {
    if uuid::Uuid::parse_str(empresa_id).is_err() {
        return Err(ApiError::dominio(
            "EMPRESA_INVALIDA",
            "El id de empresa no es un UUID valido.",
        ));
    }
    Ok(())
}

fn validar_negocio(negocio_id: &str) -> Result<(), ApiError> {
    if uuid::Uuid::parse_str(negocio_id).is_err() {
        return Err(ApiError::dominio(
            "NEGOCIO_INVALIDO",
            "El id de negocio no es un UUID valido.",
        ));
    }
    Ok(())
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

impl Capacidades {
    pub async fn listar_negocio(
        &self,
        empresa_id: &str,
        negocio_id: &str,
        actor: &UsuarioContexto,
    ) -> Result<Vec<EmpresaCapacidad>, ApiError> {
        validar_empresa(empresa_id)?;
        validar_negocio(negocio_id)?;
        self.validar_acceso_negocio(empresa_id, negocio_id, actor, "capacidades.leer")
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT c.codigo AS capacidad_codigo, c.nombre, c.modulo,
                    nc.activo, nc.config, nc.actualizado_en
               FROM identity.negocio_capacidades nc
               JOIN identity.capacidades c ON c.codigo = nc.capacidad_codigo
              WHERE nc.negocio_id = $1::uuid
              ORDER BY c.modulo, c.codigo",
        )
        .bind(negocio_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas.iter().map(fila_a_capacidad).collect()
    }

    pub async fn actualizar_negocio(
        &self,
        empresa_id: &str,
        negocio_id: &str,
        capacidad_codigo: &str,
        activo: bool,
        config: Option<Value>,
        actor: &UsuarioContexto,
    ) -> Result<EmpresaCapacidad, ApiError> {
        validar_empresa(empresa_id)?;
        validar_negocio(negocio_id)?;
        self.validar_acceso_negocio(empresa_id, negocio_id, actor, "capacidades.gestionar")
            .await?;
        let codigo = capacidad_codigo.trim().to_lowercase();
        if codigo.is_empty() {
            return Err(ApiError::dominio(
                "CAPACIDAD_INVALIDA",
                "La capacidad es obligatoria.",
            ));
        }
        let config_json = config;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let existe = sqlx::query(
            "SELECT c.codigo
               FROM identity.capacidades c
               JOIN identity.negocios n ON n.id = $1::uuid AND n.empresa_id = $2::uuid
               JOIN identity.vertical_capacidades vc
                 ON vc.vertical_codigo = n.vertical_id
                AND vc.capacidad_codigo = c.codigo
              WHERE c.codigo = $3 AND c.activo = true",
        )
        .bind(negocio_id)
        .bind(empresa_id)
        .bind(&codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if existe.is_none() {
            return Err(ApiError::conflicto(format!(
                "La capacidad \"{codigo}\" no existe o no aplica a la vertical del negocio."
            )));
        }
        let fila = sqlx::query(
            "INSERT INTO identity.negocio_capacidades AS nc
               (negocio_id, capacidad_codigo, activo, config, actualizado_en)
             VALUES ($1::uuid, $2, $3, COALESCE($4::jsonb, '{}'::jsonb), NOW())
             ON CONFLICT (negocio_id, capacidad_codigo) DO UPDATE
               SET activo = EXCLUDED.activo,
                   config = COALESCE($4::jsonb, nc.config),
                   actualizado_en = NOW()
             RETURNING capacidad_codigo, activo, config, actualizado_en,
               (SELECT nombre FROM identity.capacidades WHERE codigo = $2) AS nombre,
               (SELECT modulo FROM identity.capacidades WHERE codigo = $2) AS modulo",
        )
        .bind(negocio_id)
        .bind(&codigo)
        .bind(activo)
        .bind(config_json.clone())
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo actualizar la capacidad del negocio."))?;
        // La fila de empresa queda como resumen (union) para clientes legados.
        sqlx::query(
            "UPDATE identity.empresa_capacidades ec
                SET activo = (
                      SELECT COALESCE(bool_or(nc.activo), false)
                        FROM identity.negocio_capacidades nc
                        JOIN identity.negocios n ON n.id = nc.negocio_id
                       WHERE n.empresa_id = $1::uuid
                         AND nc.capacidad_codigo = $2
                    ),
                    actualizado_en = NOW()
              WHERE ec.empresa_id = $1::uuid AND ec.capacidad_codigo = $2",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            Some(negocio_id),
            "capacidad_negocio.actualizada",
            "capacidad",
            Some(&codigo),
            serde_json::json!({ "activo": activo }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        fila_a_capacidad(&fila)
    }

    async fn validar_acceso_negocio(
        &self,
        empresa_id: &str,
        negocio_id: &str,
        actor: &UsuarioContexto,
        permiso: &str,
    ) -> Result<(), ApiError> {
        if actor.es_admin_global() {
            return Ok(());
        }
        if actor.tenant_id.as_deref() != Some(empresa_id) {
            return Err(ApiError::acceso_denegado(
                "No tienes acceso a esta empresa.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT 1
               FROM identity.negocios n
               JOIN identity.asignaciones_personal a
                 ON a.empresa_id = n.empresa_id AND a.estado = 'activa'
               JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
              WHERE n.id = $1::uuid AND n.empresa_id = $2::uuid
                AND a.usuario_id = $3::uuid
                AND (r.negocio_id IS NULL OR r.negocio_id = n.id)
                AND (r.codigo = 'empresario' OR r.permisos->'acciones' ? $4)
              LIMIT 1",
        )
        .bind(negocio_id)
        .bind(empresa_id)
        .bind(&actor.user_id)
        .bind(permiso)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::acceso_denegado(
                "Tu rol no permite gestionar las capacidades de este negocio.",
            ));
        }
        Ok(())
    }

    /// Lectura interna (servicio→servicio): capacidades por empresa y, si se
    /// informa, por negocio. Sin `negocio_id` conserva el contrato anterior.
    pub async fn activas_de_empresas(
        &self,
        empresa_ids: &[String],
        capacidades: &[String],
        negocio_id: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        if empresa_ids.is_empty() || capacidades.is_empty() {
            return Ok(Vec::new());
        }
        let mut tx = self.db.transaccion_servicio().await.map_err(interno)?;
        let filas = if let Some(negocio_id) = negocio_id {
            let tiene_config: bool = sqlx::query_scalar(
                "SELECT EXISTS (
                   SELECT 1 FROM identity.negocio_capacidades WHERE negocio_id = $1::uuid
                 )",
            )
            .bind(negocio_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?;
            if tiene_config {
                sqlx::query(
                    "SELECT jsonb_build_object(
                              'empresa_id', n.empresa_id,
                              'capacidad_codigo', nc.capacidad_codigo,
                              'activo', nc.activo
                            ) AS datos
                       FROM identity.negocio_capacidades nc
                       JOIN identity.negocios n ON n.id = nc.negocio_id
                      WHERE nc.negocio_id = $1::uuid
                        AND n.empresa_id = ANY($2::uuid[])
                        AND nc.capacidad_codigo = ANY($3::text[])",
                )
                .bind(negocio_id)
                .bind(empresa_ids)
                .bind(capacidades)
                .fetch_all(&mut *tx)
                .await
                .map_err(interno)?
            } else {
                // Negocio legacy sin configuracion propia: se mantiene el
                // comportamiento anterior por empresa.
                sqlx::query(
                    "SELECT jsonb_build_object(
                              'empresa_id', ec.empresa_id,
                              'capacidad_codigo', ec.capacidad_codigo,
                              'activo', ec.activo
                            ) AS datos
                       FROM identity.empresa_capacidades ec
                       JOIN identity.negocios n ON n.empresa_id = ec.empresa_id
                      WHERE n.id = $1::uuid
                        AND ec.empresa_id = ANY($2::uuid[])
                        AND ec.capacidad_codigo = ANY($3::text[])",
                )
                .bind(negocio_id)
                .bind(empresa_ids)
                .bind(capacidades)
                .fetch_all(&mut *tx)
                .await
                .map_err(interno)?
            }
        } else {
            sqlx::query(
                "SELECT jsonb_build_object(
                          'empresa_id', empresa_id,
                          'capacidad_codigo', capacidad_codigo,
                          'activo', activo
                        ) AS datos
                   FROM identity.empresa_capacidades
                  WHERE empresa_id = ANY($1::uuid[])
                    AND capacidad_codigo = ANY($2::text[])",
            )
            .bind(empresa_ids)
            .bind(capacidades)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?
        };
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila.try_get("datos").map_err(interno))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_rechazar_un_empresa_id_que_no_es_uuid() {
        assert!(validar_empresa("no-es-uuid").is_err());
        assert!(validar_empresa("11111111-1111-4111-8111-111111111111").is_ok());
    }

    #[test]
    fn debe_conservar_el_shape_de_la_capacidad() {
        let capacidad = EmpresaCapacidad {
            capacidad_codigo: "catalogo_items".into(),
            nombre: "Catálogo".into(),
            modulo: "catalog".into(),
            activo: true,
            config: serde_json::json!({}),
            actualizado_en: "2026-11-13T00:00:00.000Z".into(),
        };
        assert_eq!(capacidad.capacidad_codigo, "catalogo_items");
    }
}
