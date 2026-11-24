//! Auditoría de cambios sensibles por empresa y negocio (ADR-0021).

use crate::auth::UsuarioContexto;
use crate::db::Db;
use crate::errors::ApiError;
use serde_json::{json, Value};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;

pub struct Auditoria {
    db: Arc<Db>,
}

impl Auditoria {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(
        &self,
        actor: &UsuarioContexto,
        empresa_id: &str,
        negocio_id: Option<&str>,
        accion: Option<&str>,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        if !actor.es_admin_global() {
            if actor.tenant_id.as_deref() != Some(empresa_id) {
                return Err(ApiError::acceso_denegado(
                    "No tienes acceso a la auditoría de esta empresa.",
                ));
            }
            if actor.rol != "empresario"
                && !actor
                    .permisos
                    .iter()
                    .any(|permiso| permiso == "auditoria.leer")
            {
                return Err(ApiError::acceso_denegado(
                    "Tu rol no permite consultar la auditoría.",
                ));
            }
        }
        let negocio_id = actor.negocio_id.as_deref().or(negocio_id);
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT to_jsonb(a) AS datos
               FROM identity.auditoria a
              WHERE a.empresa_id = $1::uuid
                AND ($2::uuid IS NULL OR a.negocio_id = $2::uuid)
                AND ($3::text IS NULL OR a.accion = $3)
                AND ($4::timestamptz IS NULL OR a.creado_en >= $4::timestamptz)
                AND ($5::timestamptz IS NULL OR a.creado_en <= $5::timestamptz)
              ORDER BY a.creado_en DESC
              LIMIT 500",
        )
        .bind(empresa_id)
        .bind(negocio_id)
        .bind(accion)
        .bind(desde)
        .bind(hasta)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| fila.try_get("datos").map_err(interno))
            .collect()
    }

    /// Aplica la retención configurada a `identity.auditoria`.
    ///
    /// Elimina eventos más antiguos que `dias`; `0` desactiva la limpieza.
    /// La purga de tenants ya anonimiza sus datos, pero los eventos de
    /// auditoría deben seguir una política legal explícita.
    pub async fn purgar_retencion(&self, dias: i64, modo: &str) -> Result<Value, ApiError> {
        if dias <= 0 {
            return Ok(json!({
                "retencion_dias": dias,
                "modo": modo,
                "desactivado": true,
                "afectados": 0,
            }));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let afectados = if modo == "eliminar" {
            sqlx::query(
                "DELETE FROM identity.auditoria
                  WHERE creado_en < NOW() - ($1::int * INTERVAL '1 day')",
            )
            .bind(dias as i32)
            .execute(&mut *tx)
            .await
            .map_err(interno)?
            .rows_affected()
        } else {
            // Conserva el rastro legal (acción, recurso, fechas) y redacta PII.
            sqlx::query(
                "UPDATE identity.auditoria
                    SET actor_usuario_id = NULL, request_id = NULL, datos = '{}'::jsonb
                  WHERE creado_en < NOW() - ($1::int * INTERVAL '1 day')",
            )
            .bind(dias as i32)
            .execute(&mut *tx)
            .await
            .map_err(interno)?
            .rows_affected()
        };
        tx.commit().await.map_err(interno)?;
        Ok(json!({
            "retencion_dias": dias,
            "modo": modo,
            "desactivado": false,
            "afectados": afectados,
        }))
    }
}

/// Inserta un evento dentro de la transacción del cambio auditado.
#[allow(clippy::too_many_arguments)]
pub async fn registrar(
    tx: &mut Transaction<'_, Postgres>,
    actor_usuario_id: &str,
    empresa_id: &str,
    negocio_id: Option<&str>,
    accion: &str,
    recurso: &str,
    recurso_id: Option<&str>,
    datos: Value,
) -> Result<(), ApiError> {
    let request_id = crate::logs::contexto_actual().map(|ctx| ctx.request_id);
    let actor = uuid::Uuid::parse_str(actor_usuario_id).ok();
    sqlx::query(
        "INSERT INTO identity.auditoria
           (empresa_id, negocio_id, actor_usuario_id, accion, recurso, recurso_id,
            request_id, datos)
         VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8::jsonb)",
    )
    .bind(empresa_id)
    .bind(negocio_id)
    .bind(actor)
    .bind(accion)
    .bind(recurso)
    .bind(recurso_id)
    .bind(request_id)
    .bind(datos.to_string())
    .execute(&mut **tx)
    .await
    .map_err(interno)?;
    Ok(())
}

pub fn datos_basicos(codigo: &str, nombre: &str) -> Value {
    json!({ "codigo": codigo, "nombre": nombre })
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.auditoria",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}
