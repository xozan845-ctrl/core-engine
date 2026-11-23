//! Acceso a PostgreSQL con el mismo contrato que `PgService`.
//!
//! - Pool de 10 conexiones.
//! - Toda consulta corre en transacción y, si hay contexto de usuario, inyecta
//!   los claims en `request.jwt.claims` para que RLS aísle por tenant (R-DB-2).

use crate::logs::contexto_actual;
use chrono::{DateTime, NaiveDate, SecondsFormat, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Row, Transaction};

/// Conexiones máximas por servicio. El presupuesto total (servicios x pool)
/// debe caber en `max_connections` de PostgreSQL o pasar por PgBouncer.
fn pool_max() -> u32 {
    std::env::var("DATABASE_POOL_MAX")
        .ok()
        .and_then(|valor| valor.parse::<u32>().ok())
        .filter(|valor| *valor > 0)
        .unwrap_or(4)
}

pub struct Db {
    pool: PgPool,
}

impl Db {
    pub async fn conectar(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(pool_max())
            // Claim por defecto para jobs y conexiones sin usuario; las
            // transacciones HTTP lo sobreescriben con el usuario real.
            .after_connect(|conn, _| {
                Box::pin(async move {
                    sqlx::query("SELECT set_config('request.jwt.claims', $1, false)")
                        .bind(serde_json::json!({ "rol": "super_admin" }).to_string())
                        .execute(conn)
                        .await?;
                    Ok::<(), sqlx::Error>(())
                })
            })
            .connect(url)
            .await?;
        sqlx::query("SELECT 1").execute(&pool).await?;
        Ok(Self { pool })
    }

    /// Transacción con claims de servicio (super_admin) para jobs de fondo que
    /// no tienen un usuario en contexto y filtran explícitamente por tenant.
    pub async fn transaccion_servicio(
        &self,
    ) -> Result<sqlx::Transaction<'static, sqlx::Postgres>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT set_config('request.jwt.claims', $1, true)")
            .bind(serde_json::json!({ "rol": "super_admin" }).to_string())
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Transacción con los claims de RLS del contexto actual.
    pub async fn transaccion(&self) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
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

/// Fecha `date` de PostgreSQL tal como la serializa node-postgres: medianoche
/// en la zona horaria del proceso (`TZ`, por defecto `America/Managua`) y luego
/// ISO UTC. Es el comportamiento observable actual del contrato.
pub fn fecha_local_iso(fecha: NaiveDate) -> String {
    let zona = std::env::var("TZ")
        .ok()
        .and_then(|valor| valor.parse::<Tz>().ok())
        .unwrap_or(chrono_tz::America::Managua);
    let medianoche = fecha
        .and_hms_opt(0, 0, 0)
        .expect("medianoche siempre existe");
    let local = zona
        .from_local_datetime(&medianoche)
        .single()
        .unwrap_or_else(|| zona.from_utc_datetime(&medianoche));
    iso_ms(local.with_timezone(&Utc))
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

pub fn fecha(fila: &sqlx::postgres::PgRow, columna: &str) -> Result<String, sqlx::Error> {
    fila.try_get::<NaiveDate, _>(columna).map(fecha_local_iso)
}

pub fn fecha_opcional(
    fila: &sqlx::postgres::PgRow,
    columna: &str,
) -> Result<Option<String>, sqlx::Error> {
    fila.try_get::<Option<NaiveDate>, _>(columna)
        .map(|valor| valor.map(fecha_local_iso))
}

pub fn hora(fila: &sqlx::postgres::PgRow, columna: &str) -> Result<String, sqlx::Error> {
    fila.try_get::<chrono::NaiveTime, _>(columna)
        .map(|valor| valor.format("%H:%M:%S").to_string())
}

/// `true` si el error es una exclusión de solapamiento (23P01) o unique (23505).
pub fn es_conflicto_periodo(error: &sqlx::Error) -> bool {
    matches!(error, sqlx::Error::Database(db) if matches!(db.code().as_deref(), Some("23P01") | Some("23505")))
}

/// Serializa `jsonb` (sqlx devuelve `Value`).
pub fn jsonb(fila: &sqlx::postgres::PgRow, columna: &str) -> Result<Value, sqlx::Error> {
    fila.try_get::<Value, _>(columna)
}

/// Error de base de datos sin filtrar detalles internos (R-C-8).
pub fn error_db(_error: sqlx::Error) -> crate::errors::ApiError {
    crate::errors::ApiError::interno()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_formatear_timestamp_iso_con_milisegundos_y_z() {
        let fecha = Utc
            .with_ymd_and_hms(2026, 11, 11, 19, 9, 9)
            .single()
            .expect("fecha");
        assert_eq!(iso_ms(fecha), "2026-11-11T19:09:09.000Z");
    }

    #[test]
    fn debe_serializar_una_fecha_date_como_medianoche_local() {
        let fecha = NaiveDate::from_ymd_opt(2026, 11, 11).expect("fecha");
        // America/Managua es UTC-6 todo el año.
        assert_eq!(fecha_local_iso(fecha), "2026-11-11T06:00:00.000Z");
    }
}
