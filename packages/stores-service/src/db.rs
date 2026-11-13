//! Acceso a PostgreSQL con el mismo contrato que `PgService`.
//!
//! - Pool de 10 conexiones.
//! - Toda consulta corre en transacción y, si hay contexto de usuario, inyecta
//!   los claims en `request.jwt.claims` para que RLS aísle por tenant (R-DB-2).
//! - Las filas se convierten a JSON con los mismos tipos que serializa
//!   `node-postgres` + `JSON.stringify` (UUID/texto como string, timestamptz
//!   como ISO 8601 con milisegundos, jsonb como objeto, etc.).

use crate::logs::contexto_actual;
use chrono::{DateTime, NaiveDateTime, SecondsFormat, Utc};
use serde_json::{json, Map, Value};
use sqlx::postgres::{PgPoolOptions, PgRow};
use sqlx::{Column, PgPool, Postgres, Row, Transaction, TypeInfo, ValueRef};

pub struct Db {
    pool: PgPool,
}

impl Db {
    pub async fn conectar(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await?;
        sqlx::query("SELECT 1").execute(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn cerrar(&self) {
        self.pool.close().await;
    }

    /// Transacción con los claims de RLS del contexto actual.
    pub async fn transaccion(&self) -> Result<Transaction<'static, Postgres>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        if let Some(ctx) = contexto_actual() {
            if let Some(sub) = ctx.usuario_id.filter(|valor| !valor.is_empty()) {
                let mut claims = Map::new();
                claims.insert("sub".into(), Value::String(sub));
                claims.insert(
                    "rol".into(),
                    Value::String(ctx.rol.unwrap_or_else(|| "anon".to_string())),
                );
                if let Some(tenant) = ctx.tenant_id {
                    claims.insert("tenant_id".into(), Value::String(tenant));
                }
                sqlx::query("SELECT set_config('request.jwt.claims', $1, true)")
                    .bind(Value::Object(claims).to_string())
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

/// Convierte una fila de PostgreSQL a JSON con los tipos de node-postgres.
pub fn fila_a_json(fila: &PgRow) -> Result<Value, sqlx::Error> {
    let mut objeto = Map::new();
    for (indice, columna) in fila.columns().iter().enumerate() {
        let nombre = columna.name().to_string();
        let crudo = fila.try_get_raw(indice)?;
        let valor = if crudo.is_null() {
            Value::Null
        } else {
            let tipo = columna.type_info().name();
            match tipo {
                "UUID" => Value::String(fila.try_get::<uuid::Uuid, _>(indice)?.to_string()),
                "TEXT" | "VARCHAR" | "BPCHAR" | "NAME" | "UNKNOWN" => {
                    Value::String(fila.try_get::<String, _>(indice)?)
                }
                "BOOL" => Value::Bool(fila.try_get::<bool, _>(indice)?),
                "INT2" => json!(fila.try_get::<i16, _>(indice)?),
                "INT4" => json!(fila.try_get::<i32, _>(indice)?),
                // node-postgres devuelve int8 como string; el contrato de
                // stores solo expone conteos con `::int`, pero se conserva la
                // fidelidad para cualquier consulta futura.
                "INT8" => Value::String(fila.try_get::<i64, _>(indice)?.to_string()),
                "FLOAT4" => json!(fila.try_get::<f32, _>(indice)?),
                "FLOAT8" => json!(fila.try_get::<f64, _>(indice)?),
                "TIMESTAMPTZ" => json!(iso_ms(fila.try_get::<DateTime<Utc>, _>(indice)?)),
                "TIMESTAMP" => {
                    let fecha = fila.try_get::<NaiveDateTime, _>(indice)?;
                    json!(iso_ms(DateTime::<Utc>::from_naive_utc_and_offset(
                        fecha, Utc
                    )))
                }
                "JSON" | "JSONB" => fila.try_get::<Value, _>(indice)?,
                _ => Value::Null,
            }
        };
        objeto.insert(nombre, valor);
    }
    Ok(Value::Object(objeto))
}

/// Traduce un error de infraestructura a 500 y lo deja en el log (sin filtrar
/// detalles internos al cliente).
pub fn interno(error: impl std::fmt::Display) -> crate::errors::ApiError {
    crate::logs::error(
        "db",
        crate::campos! { "msg": "error de infraestructura", "err": error.to_string() },
    );
    crate::errors::ApiError::interno()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_formatear_timestamp_iso_con_milisegundos_y_z() {
        use chrono::TimeZone;
        let fecha = Utc
            .with_ymd_and_hms(2026, 11, 13, 19, 9, 9)
            .single()
            .expect("fecha");
        assert_eq!(iso_ms(fecha), "2026-11-13T19:09:09.000Z");
    }
}
