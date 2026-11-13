//! Capacidades por empresa (R-AR-5).
//!
//! Réplica de `capacidades/capacidades.service.ts`: el admin global gestiona
//! cualquier empresa; el empresario solo su tenant y solo si sigue siendo
//! miembro activo con rol `empresario`.

use crate::auth::UsuarioContexto;
use crate::db::{iso_ms, Db};
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
        .bind(config_json)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo actualizar la capacidad."))?;
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

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
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
