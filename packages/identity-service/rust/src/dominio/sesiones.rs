//! Persistencia de sesiones (refresh tokens) en `identity.sesiones`.
//!
//! Réplica de `auth/sesiones.repository.ts` (R-GW-4, ADR-18): soft delete por
//! `revocada_en`; el refresh solo renueva si la sesión sigue activa.

use crate::db::Db;
use crate::errors::ApiError;
use chrono::{DateTime, Utc};
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
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        sqlx::query(
            "INSERT INTO identity.sesiones (id, usuario_id, jti, expira_en)
             VALUES (gen_random_uuid(), $1::uuid, $2::uuid, $3)",
        )
        .bind(usuario_id)
        .bind(jti)
        .bind(expira_en)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(())
    }

    pub async fn activa_por_jti(&self, jti: &str) -> Result<Option<SesionActiva>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, usuario_id, jti
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
        }))
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
        };
        assert_eq!(sesion.usuario_id, "u-1");
        assert_eq!(sesion.jti, "jti-1");
    }
}
