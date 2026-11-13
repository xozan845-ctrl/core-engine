//! Acceso a PostgreSQL con el mismo contrato que `PgService`.
//!
//! - Pool de 10 conexiones.
//! - Toda consulta corre en transacción y, si hay contexto de usuario, inyecta
//!   los claims en `request.jwt.claims` para que RLS aísle por tenant (R-DB-2).

use crate::logs::contexto_actual;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::Row;

pub struct Db {
    pool: sqlx::PgPool,
}

impl Db {
    /// Pool diferido: no abre conexiones hasta la primera consulta. Permite
    /// construir el estado de pruebas HTTP sin PostgreSQL.
    pub fn perezoso(url: &str) -> Self {
        Self {
            pool: PgPoolOptions::new()
                .max_connections(1)
                .connect_lazy(url)
                .expect("url de postgres"),
        }
    }

    pub async fn conectar(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await?;
        sqlx::query("SELECT 1").execute(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    /// Transacción con los claims de RLS del contexto actual.
    pub async fn transaccion(
        &self,
    ) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        if let Some(ctx) = contexto_actual() {
            if let Some(sub) = ctx.usuario_id.filter(|valor| !valor.is_empty()) {
                let claims = json!({
                    "sub": sub,
                    "rol": ctx.rol.unwrap_or_else(|| "anon".to_string()),
                    "tenant_id": ctx.tenant_id,
                });
                sqlx::query("SELECT set_config('request.jwt.claims', $1, true)")
                    .bind(claims.to_string())
                    .execute(&mut *tx)
                    .await?;
            }
        }
        Ok(tx)
    }
}

/// Timestamp ISO 8601 UTC con milisegundos y `Z` (R-C-4), como `toISOString()`.
pub fn iso_ms(fecha: DateTime<Utc>) -> String {
    fecha.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// IDs UUID serializados como string (node-postgres devuelve strings).
pub fn uuid_texto(fila: &sqlx::postgres::PgRow, columna: &str) -> Result<String, sqlx::Error> {
    fila.try_get::<uuid::Uuid, _>(columna)
        .map(|valor| valor.to_string())
}

/// UUID opcional (columnas nullable) serializado como string o `null`.
pub fn uuid_opcional(
    fila: &sqlx::postgres::PgRow,
    columna: &str,
) -> Result<Option<String>, sqlx::Error> {
    fila.try_get::<Option<uuid::Uuid>, _>(columna)
        .map(|valor| valor.map(|uuid| uuid.to_string()))
}

pub fn timestamp(fila: &sqlx::postgres::PgRow, columna: &str) -> Result<String, sqlx::Error> {
    fila.try_get::<DateTime<Utc>, _>(columna).map(iso_ms)
}

pub fn timestamp_opcional(
    fila: &sqlx::postgres::PgRow,
    columna: &str,
) -> Result<Option<String>, sqlx::Error> {
    fila.try_get::<Option<DateTime<Utc>>, _>(columna)
        .map(|valor| valor.map(iso_ms))
}

/// Serializa `jsonb` (sqlx devuelve `Value`).
pub fn jsonb(fila: &sqlx::postgres::PgRow, columna: &str) -> Result<Value, sqlx::Error> {
    fila.try_get::<Value, _>(columna)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn debe_formatear_timestamp_iso_con_milisegundos_y_z() {
        let fecha = Utc
            .with_ymd_and_hms(2026, 11, 11, 19, 9, 9)
            .single()
            .expect("fecha");
        assert_eq!(iso_ms(fecha), "2026-11-11T19:09:09.000Z");
    }
}
