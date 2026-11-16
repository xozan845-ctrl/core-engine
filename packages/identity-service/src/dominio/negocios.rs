//! Negocios: unidad operativa dentro de una empresa (ADR-0019).
//!
//! La empresa sigue siendo el tenant fiscal y el límite de RLS. Cada negocio
//! aporta su vertical y su pack; una empresa simple nace con el negocio
//! `principal` y solo un `empresario` activo puede añadir más.

use crate::auth::UsuarioContexto;
use crate::db::Db;
use crate::dominio::auditoria;
use crate::errors::ApiError;
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub struct CrearNegocio {
    pub codigo: String,
    pub nombre: String,
    pub vertical_id: String,
    pub pack_codigo: String,
    pub pack_version: Option<i32>,
}

pub struct Negocios {
    db: Arc<Db>,
}

impl Negocios {
    pub fn nuevo(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        self.assert_acceso(actor, empresa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(n) || jsonb_build_object(
                      'vertical_nombre', v.nombre,
                      'pack_nombre', p.nombre
                    ) AS datos
               FROM identity.negocios n
               JOIN identity.verticales v ON v.codigo = n.vertical_id
               LEFT JOIN identity.packs p
                 ON p.codigo = n.pack_codigo AND p.version = n.pack_version
              WHERE n.empresa_id = $1::uuid
              ORDER BY n.es_predeterminado DESC, n.nombre",
        )
        .bind(empresa_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila.try_get("datos").map_err(interno))
            .collect()
    }

    /// Añade un negocio a una empresa. El negocio principal se crea junto con
    /// la empresa; a partir del segundo se exige un `empresario` activo de la
    /// empresa (regla de negocio de ADR-0019).
    pub async fn crear(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        datos: CrearNegocio,
    ) -> Result<Value, ApiError> {
        let codigo = datos.codigo.trim().to_lowercase();
        if !codigo_valido(&codigo) {
            return Err(ApiError::dominio(
                "NEGOCIO_INVALIDO",
                "El código debe empezar por letra y usar minúsculas, números o guion bajo.",
            ));
        }
        let nombre = datos.nombre.trim();
        if !(2..=120).contains(&nombre.chars().count()) {
            return Err(ApiError::dominio(
                "NEGOCIO_INVALIDO",
                "El nombre debe tener entre 2 y 120 caracteres.",
            ));
        }
        let vertical = datos.vertical_id.trim().to_lowercase();
        if vertical.is_empty() {
            return Err(ApiError::dominio(
                "VERTICAL_REQUERIDA",
                "Selecciona una vertical para el negocio.",
            ));
        }
        let pack_codigo = datos.pack_codigo.trim().to_lowercase();
        if pack_codigo.is_empty() {
            return Err(ApiError::dominio(
                "PACK_REQUERIDO",
                "Selecciona un pack para el negocio.",
            ));
        }

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        self.assert_acceso_en_tx(&mut tx, actor, empresa_id).await?;
        let existentes: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM identity.negocios WHERE empresa_id = $1::uuid",
        )
        .bind(empresa_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(interno)?;
        if existentes > 0 {
            self.assert_empresario_en_tx(&mut tx, actor, empresa_id)
                .await?;
        }

        let pack = sqlx::query(
            "SELECT codigo, version, vertical_codigo
               FROM identity.packs
              WHERE codigo = $1 AND ($2::int IS NULL OR version = $2) AND activo = true
              ORDER BY version DESC
              LIMIT 1",
        )
        .bind(&pack_codigo)
        .bind(datos.pack_version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| {
            ApiError::dominio(
                "PACK_INVALIDO",
                format!("El pack \"{pack_codigo}\" no existe o está inactivo."),
            )
        })?;
        let pack_vertical: String = pack.try_get("vertical_codigo").map_err(interno)?;
        if pack_vertical != vertical {
            return Err(ApiError::dominio(
                "PACK_VERTICAL_INCOMPATIBLE",
                "El pack no corresponde a la vertical del negocio.",
            ));
        }
        let pack_version: i32 = pack.try_get("version").map_err(interno)?;

        let fila = sqlx::query(
            "INSERT INTO identity.negocios AS n
               (empresa_id, codigo, nombre, vertical_id, pack_codigo, pack_version, creado_por)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7::uuid)
             RETURNING to_jsonb(n) AS datos",
        )
        .bind(empresa_id)
        .bind(&codigo)
        .bind(nombre)
        .bind(&vertical)
        .bind(&pack_codigo)
        .bind(pack_version)
        .bind(&actor.user_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_codigo)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear el negocio."))?;
        let datos: Value = fila.try_get("datos").map_err(interno)?;
        let negocio_id = datos
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(ApiError::interno)?;

        // La empresa conserva el resumen legado; la autoridad del negocio es
        // `negocio_capacidades`.
        sqlx::query(
            "INSERT INTO identity.empresa_capacidades
               (empresa_id, capacidad_codigo, config, activo)
             SELECT $1::uuid, capacidad_codigo, config, activo
               FROM identity.pack_capacidades
              WHERE pack_codigo = $2 AND pack_version = $3
             ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING",
        )
        .bind(empresa_id)
        .bind(&pack_codigo)
        .bind(pack_version)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.negocio_capacidades
               (negocio_id, capacidad_codigo, config, activo)
             SELECT $1::uuid, capacidad_codigo, config, activo
               FROM identity.pack_capacidades
              WHERE pack_codigo = $2 AND pack_version = $3
             ON CONFLICT (negocio_id, capacidad_codigo) DO NOTHING",
        )
        .bind(negocio_id)
        .bind(&pack_codigo)
        .bind(pack_version)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;

        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            Some(negocio_id),
            "negocio.creado",
            "negocio",
            Some(negocio_id),
            auditoria::datos_basicos(&codigo, nombre),
        )
        .await?;

        aplicar_plantilla_en_tx(&mut tx, empresa_id, negocio_id, &vertical, &actor.user_id).await?;
        crear_ubicacion_principal(&mut tx, empresa_id, negocio_id).await?;

        tx.commit().await.map_err(interno)?;
        Ok(datos)
    }

    pub async fn plantillas(&self, vertical_id: &str) -> Result<Vec<Value>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(p) AS datos
               FROM identity.plantillas_roles p
              WHERE p.vertical_id = $1 AND p.activo = true
              ORDER BY p.nombre",
        )
        .bind(vertical_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila.try_get("datos").map_err(interno))
            .collect()
    }

    pub async fn aplicar_plantilla(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        negocio_id: &str,
    ) -> Result<usize, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        self.assert_acceso_en_tx(&mut tx, actor, empresa_id).await?;
        self.assert_empresario_en_tx(&mut tx, actor, empresa_id)
            .await?;
        let vertical = sqlx::query_scalar::<_, String>(
            "SELECT vertical_id FROM identity.negocios
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND estado = 'activo'",
        )
        .bind(negocio_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Negocio", negocio_id))?;
        let creados =
            aplicar_plantilla_en_tx(&mut tx, empresa_id, negocio_id, &vertical, &actor.user_id)
                .await?;
        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            Some(negocio_id),
            "plantilla.aplicada",
            "negocio",
            Some(negocio_id),
            json!({ "vertical_id": vertical, "roles_creados": creados }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        Ok(creados)
    }

    pub async fn archivar(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        negocio_id: &str,
        estado: &str,
    ) -> Result<Value, ApiError> {
        let (estado, archivado) = match estado {
            "activo" => ("activo", false),
            "suspendido" => ("suspendido", false),
            "archivado" => ("inactivo", true),
            _ => {
                return Err(ApiError::dominio(
                    "ESTADO_INVALIDO",
                    "Usa activo, suspendido o archivado.",
                ))
            }
        };
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        self.assert_acceso_en_tx(&mut tx, actor, empresa_id).await?;
        self.assert_empresario_en_tx(&mut tx, actor, empresa_id)
            .await?;
        let fila = sqlx::query(
            "UPDATE identity.negocios AS n
                SET estado = $1,
                    archivado_en = CASE WHEN $2 THEN NOW() ELSE NULL END,
                    actualizado_en = NOW()
              WHERE n.id = $3::uuid AND n.empresa_id = $4::uuid
              RETURNING to_jsonb(n) AS datos",
        )
        .bind(estado)
        .bind(archivado)
        .bind(negocio_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::no_encontrado("Negocio", negocio_id))?;
        auditoria::registrar(
            &mut tx,
            &actor.user_id,
            empresa_id,
            Some(negocio_id),
            "negocio.estado",
            "negocio",
            Some(negocio_id),
            json!({ "estado": estado, "archivado": archivado }),
        )
        .await?;
        tx.commit().await.map_err(interno)?;
        fila.try_get("datos").map_err(interno)
    }

    /// Negocios a los que el actor puede cambiar su sesión.
    pub async fn disponibles(&self, actor: &UsuarioContexto) -> Result<Vec<Value>, ApiError> {
        let empresa_id = actor.tenant()?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(n) AS datos
               FROM identity.negocios n
              WHERE n.empresa_id = $1::uuid AND n.estado = 'activo'
                AND EXISTS (
                  SELECT 1
                    FROM identity.asignaciones_personal a
                    JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
                   WHERE a.usuario_id = $2::uuid AND a.empresa_id = n.empresa_id
                     AND a.estado = 'activa'
                     AND (a.negocio_id = n.id OR r.negocio_id = n.id
                          OR (r.codigo = 'empresario' AND r.negocio_id IS NULL))
                )
              ORDER BY n.es_predeterminado DESC, n.nombre",
        )
        .bind(empresa_id)
        .bind(&actor.user_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila.try_get("datos").map_err(interno))
            .collect()
    }

    /// Devuelve el negocio indicado o el predeterminado de la empresa.
    /// `None` solo si la empresa aún no tiene negocio (tenant legacy).
    pub async fn resolver(
        &self,
        empresa_id: &str,
        negocio_id: Option<&str>,
    ) -> Result<Option<String>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = match negocio_id {
            Some(id) => sqlx::query(
                "SELECT id FROM identity.negocios
                  WHERE id = $1::uuid AND empresa_id = $2::uuid",
            )
            .bind(id)
            .bind(empresa_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
            None => sqlx::query(
                "SELECT id FROM identity.negocios
                  WHERE empresa_id = $1::uuid AND es_predeterminado
                  LIMIT 1",
            )
            .bind(empresa_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?,
        };
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => {
                let id: uuid::Uuid = fila.try_get("id").map_err(interno)?;
                Ok(Some(id.to_string()))
            }
            None if negocio_id.is_some() => Err(ApiError::no_encontrado(
                "Negocio",
                negocio_id.unwrap_or_default(),
            )),
            None => Ok(None),
        }
    }

    async fn assert_acceso(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
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
        self.assert_acceso_en_tx(&mut tx, actor, empresa_id).await?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    async fn assert_acceso_en_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<(), ApiError> {
        if actor.es_admin_global() {
            return Ok(());
        }
        let miembro = sqlx::query(
            "SELECT 1 FROM identity.asignaciones_personal
              WHERE empresa_id = $1::uuid AND usuario_id = $2::uuid AND estado = 'activa'
              LIMIT 1",
        )
        .bind(empresa_id)
        .bind(&actor.user_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?;
        if miembro.is_none() {
            return Err(ApiError::acceso_denegado(
                "No tienes una asignación activa en esta empresa.",
            ));
        }
        Ok(())
    }

    async fn assert_empresario_en_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        actor: &UsuarioContexto,
        empresa_id: &str,
    ) -> Result<(), ApiError> {
        if actor.es_admin_global() {
            return Ok(());
        }
        let miembro = sqlx::query(
            "SELECT 1 FROM identity.asignaciones_personal a
               JOIN identity.roles_empresa r ON r.id = a.rol_empresa_id
              WHERE a.empresa_id = $1::uuid AND a.usuario_id = $2::uuid
                AND a.estado = 'activa'
                AND r.codigo = 'empresario' AND r.negocio_id IS NULL
              LIMIT 1",
        )
        .bind(empresa_id)
        .bind(&actor.user_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(interno)?;
        if miembro.is_none() {
            return Err(ApiError::dominio(
                "EMPRESARIO_REQUERIDO",
                "Solo un empresario activo de la empresa puede añadir más negocios.",
            ));
        }
        Ok(())
    }
}

async fn aplicar_plantilla_en_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa_id: &str,
    negocio_id: &str,
    vertical_id: &str,
    actor_id: &str,
) -> Result<usize, ApiError> {
    let resultado = sqlx::query(
        "INSERT INTO identity.roles_empresa
           (empresa_id, negocio_id, codigo, nombre, permisos, creado_por)
         SELECT $1::uuid, $2::uuid, p.codigo, p.nombre, p.permisos, $4::uuid
           FROM identity.plantillas_roles p
          WHERE p.vertical_id = $3 AND p.activo = true
         ON CONFLICT DO NOTHING",
    )
    .bind(empresa_id)
    .bind(negocio_id)
    .bind(vertical_id)
    .bind(actor_id)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(resultado.rows_affected() as usize)
}

async fn crear_ubicacion_principal(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa_id: &str,
    negocio_id: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO inventory.ubicaciones
           (empresa_id, negocio_id, codigo, nombre, tipo, predeterminada)
         VALUES ($1::uuid, $2::uuid,
                 'principal_' || substr(replace($2::text, '-', ''), 1, 8),
                 'Principal', 'bodega', true)
         ON CONFLICT DO NOTHING",
    )
    .bind(empresa_id)
    .bind(negocio_id)
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

fn codigo_valido(valor: &str) -> bool {
    let mut chars = valor.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&valor.len())
        && valor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn conflicto_codigo(error: sqlx::Error) -> ApiError {
    if matches!(&error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")) {
        return ApiError::conflicto("Ya existe un negocio con ese código en la empresa.");
    }
    interno(error)
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.negocios",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_el_codigo_del_negocio() {
        assert!(codigo_valido("principal"));
        assert!(codigo_valido("taller_2"));
        assert!(!codigo_valido("Principal"));
        assert!(!codigo_valido("a"));
        assert!(!codigo_valido("taller-2"));
        assert!(!codigo_valido("2taller"));
    }

    #[test]
    fn debe_serializar_el_contrato_de_creacion() {
        let datos = CrearNegocio {
            codigo: "principal".into(),
            nombre: "Principal".into(),
            vertical_id: "retail".into(),
            pack_codigo: "pack_retail".into(),
            pack_version: None,
        };
        assert_eq!(datos.vertical_id, "retail");
    }
}
