//! Persistencia de sesiones (refresh tokens) en `identity.sesiones`.
//!
//! Réplica de `auth/sesiones.repository.ts` (R-GW-4, ADR-18): soft delete por
//! `revocada_en`; el refresh solo renueva si la sesión sigue activa.

use crate::db::{iso_ms, Db};
use crate::errors::ApiError;
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

pub struct Sesiones {
    db: Arc<Db>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SesionActiva {
    pub id: String,
    pub usuario_id: String,
    pub jti: String,
    pub negocio_id: Option<String>,
}

impl Sesiones {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn crear(
        &self,
        usuario_id: &str,
        jti: &str,
        expira_en: DateTime<Utc>,
        negocio_id: Option<&str>,
        user_agent: Option<&str>,
        ip: Option<&str>,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.sesiones
               (id, usuario_id, jti, negocio_id, expira_en, user_agent, ip, ultimo_uso_en)
             VALUES (gen_random_uuid(), $1::uuid, $2::uuid, $3::uuid, $4, $5, $6, NOW())",
        )
        .bind(usuario_id)
        .bind(jti)
        .bind(negocio_id)
        .bind(expira_en)
        .bind(user_agent)
        .bind(ip)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    pub async fn activa_por_jti(&self, jti: &str) -> Result<Option<SesionActiva>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, usuario_id, jti, negocio_id
               FROM identity.sesiones
              WHERE jti = $1::uuid AND revocada_en IS NULL AND expira_en > NOW()",
        )
        .bind(jti)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(fila.map(|fila| SesionActiva {
            id: fila.get::<uuid::Uuid, _>("id").to_string(),
            usuario_id: fila.get::<uuid::Uuid, _>("usuario_id").to_string(),
            jti: fila.get::<uuid::Uuid, _>("jti").to_string(),
            negocio_id: fila
                .get::<Option<uuid::Uuid>, _>("negocio_id")
                .map(|valor| valor.to_string()),
        }))
    }

    pub async fn actualizar_uso(&self, jti: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query("UPDATE identity.sesiones SET ultimo_uso_en = NOW() WHERE jti = $1::uuid")
            .bind(jti)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    pub async fn listar_activas(&self, usuario_id: &str) -> Result<Vec<Value>, ApiError> {
        if uuid::Uuid::parse_str(usuario_id).is_err() {
            return Ok(Vec::new());
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT id, negocio_id, creada_en, expira_en, user_agent, ip, ultimo_uso_en
               FROM identity.sesiones
              WHERE usuario_id = $1::uuid
                AND revocada_en IS NULL
                AND expira_en > NOW()
              ORDER BY creada_en DESC",
        )
        .bind(usuario_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                let creada: DateTime<Utc> = fila.try_get("creada_en")?;
                let expira: DateTime<Utc> = fila.try_get("expira_en")?;
                let ultimo: Option<DateTime<Utc>> = fila.try_get("ultimo_uso_en")?;
                let negocio: Option<uuid::Uuid> = fila.try_get("negocio_id")?;
                Ok(json!({
                    "id": fila.try_get::<uuid::Uuid, _>("id")?.to_string(),
                    "negocio_id": negocio.map(|valor| valor.to_string()),
                    "creada_en": iso_ms(creada),
                    "expira_en": iso_ms(expira),
                    "user_agent": fila.try_get::<Option<String>, _>("user_agent")?,
                    "ip": fila.try_get::<Option<String>, _>("ip")?,
                    "ultimo_uso_en": ultimo.map(iso_ms),
                }))
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(interno)
    }

    pub async fn revocar_por_id(&self, usuario_id: &str, id: &str) -> Result<bool, ApiError> {
        if uuid::Uuid::parse_str(usuario_id).is_err() || uuid::Uuid::parse_str(id).is_err() {
            return Ok(false);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila: Option<uuid::Uuid> = sqlx::query_scalar(
            "UPDATE identity.sesiones
                SET revocada_en = NOW()
              WHERE id = $1::uuid AND usuario_id = $2::uuid AND revocada_en IS NULL
              RETURNING id",
        )
        .bind(id)
        .bind(usuario_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(fila.is_some())
    }

    pub async fn actualizar_negocio(
        &self,
        jti: &str,
        usuario_id: &str,
        negocio_id: &str,
    ) -> Result<bool, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado = sqlx::query(
            "UPDATE identity.sesiones SET negocio_id = $1::uuid
              WHERE jti = $2::uuid AND usuario_id = $3::uuid
                AND revocada_en IS NULL AND expira_en > NOW()",
        )
        .bind(negocio_id)
        .bind(jti)
        .bind(usuario_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(resultado.rows_affected() > 0)
    }

    pub async fn revocar(&self, jti: &str, usuario_id: &str) -> Result<u64, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado = sqlx::query(
            "UPDATE identity.sesiones SET revocada_en = NOW()
              WHERE jti = $1::uuid AND usuario_id = $2::uuid AND revocada_en IS NULL",
        )
        .bind(jti)
        .bind(usuario_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(resultado.rows_affected())
    }

    pub async fn revocar_todas(&self, usuario_id: &str) -> Result<u64, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado = sqlx::query(
            "UPDATE identity.sesiones SET revocada_en = NOW()
              WHERE usuario_id = $1::uuid AND revocada_en IS NULL",
        )
        .bind(usuario_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(resultado.rows_affected())
    }
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
    fn debe_conservar_los_identificadores_como_texto() {
        let sesion = SesionActiva {
            id: "id-1".into(),
            usuario_id: "u-1".into(),
            jti: "jti-1".into(),
            negocio_id: Some("n-1".into()),
        };
        assert_eq!(sesion.usuario_id, "u-1");
        assert_eq!(sesion.jti, "jti-1");
    }
}
