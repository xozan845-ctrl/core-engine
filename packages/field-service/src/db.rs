//! Acceso a PostgreSQL con el mismo contrato que `PgService`.
//!
//! - Pool de 10 conexiones.
//! - Toda consulta corre en transacción y, si hay contexto de usuario, inyecta
//!   los claims en `request.jwt.claims` para que RLS aísle por tenant (R-DB-2).
//! - Las filas se convierten a JSON con los mismos tipos que serializa
//!   `node-postgres` + `JSON.stringify` (UUID/texto como string, timestamptz y
//!   date como ISO 8601, jsonb como objeto, etc.).

use crate::logs::contexto_actual;
use chrono::{DateTime, NaiveDate, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use chrono_tz::Tz;
use serde_json::{json, Map, Value};
use sqlx::postgres::{PgArguments, PgPoolOptions, PgRow};
use sqlx::query::Query;
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

/// Tipo SQL de una columna del esquema `field`, usado para castear los
/// parámetros de texto y conservar el comportamiento de `node-postgres`
/// (parámetro desconocido inferido por el contexto).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoColumna {
    Uuid,
    Texto,
    Booleano,
    Entero,
    Flotante,
    Timestamptz,
    Fecha,
    Jsonb,
}

pub fn tipo_columna(columna: &str) -> TipoColumna {
    match columna {
        "id"
        | "tenant_id"
        | "ruta_asignada_id"
        | "vehiculo_asignado_id"
        | "conductor_id"
        | "ruta_activa_id"
        | "ruta_id"
        | "pedido_id"
        | "parte_id"
        | "personal_id"
        | "vehiculo_id"
        | "cliente_id" => TipoColumna::Uuid,
        "hora_check_in" | "hora_check_out" | "ubicacion_ts" | "fecha_inicio" | "fecha_fin"
        | "registrado_en" => TipoColumna::Timestamptz,
        "vencimiento_seguro" | "vencimiento_circulacion" | "fecha" => TipoColumna::Fecha,
        "personal_asignado_ids" | "fotos_justificacion" | "metricas" => TipoColumna::Jsonb,
        "completada" | "en_sede" | "monitoreo_activo" | "synced" => TipoColumna::Booleano,
        "anio" | "orden" | "minutos_retraso_real" | "duracion_minutos" => TipoColumna::Entero,
        "ubicacion_lat"
        | "ubicacion_lng"
        | "ubicacion_precision"
        | "capacidad_carga_kg"
        | "lat"
        | "lng"
        | "latitud"
        | "longitud"
        | "precision"
        | "velocidad"
        | "rumbo"
        | "distancia_al_cliente" => TipoColumna::Flotante,
        _ => TipoColumna::Texto,
    }
}

pub fn cast(tipo: TipoColumna) -> &'static str {
    match tipo {
        TipoColumna::Uuid => "::uuid",
        TipoColumna::Texto => "",
        TipoColumna::Booleano => "::boolean",
        TipoColumna::Entero => "::integer",
        TipoColumna::Flotante => "::double precision",
        TipoColumna::Timestamptz => "::timestamptz",
        TipoColumna::Fecha => "::date",
        TipoColumna::Jsonb => "::jsonb",
    }
}

/// Valor ligado a una consulta. Se construye desde JSON para conservar
/// `undefined` vs `null` en la capa de servicio.
#[derive(Debug, Clone, PartialEq)]
pub enum Valor {
    Texto(String),
    Entero(i64),
    Flotante(f64),
    Booleano(bool),
    Nulo,
}

impl Valor {
    pub fn desde_json(valor: &Value, tipo: TipoColumna) -> Self {
        match valor {
            Value::Null => Valor::Nulo,
            Value::String(texto) => Valor::Texto(texto.clone()),
            Value::Bool(bandera) => Valor::Booleano(*bandera),
            Value::Number(numero) => match tipo {
                TipoColumna::Entero => numero
                    .as_i64()
                    .map(Valor::Entero)
                    .unwrap_or_else(|| Valor::Texto(numero.to_string())),
                TipoColumna::Flotante => numero
                    .as_f64()
                    .map(Valor::Flotante)
                    .unwrap_or_else(|| Valor::Texto(numero.to_string())),
                _ => Valor::Texto(numero.to_string()),
            },
            Value::Array(_) | Value::Object(_) => Valor::Texto(valor.to_string()),
        }
    }

    pub fn desde_texto(texto: impl Into<String>) -> Self {
        Valor::Texto(texto.into())
    }

    pub fn a_sql<'q>(
        &self,
        consulta: Query<'q, Postgres, PgArguments>,
    ) -> Query<'q, Postgres, PgArguments> {
        match self {
            Valor::Texto(texto) => consulta.bind(texto.clone()),
            Valor::Entero(numero) => consulta.bind(*numero),
            Valor::Flotante(numero) => consulta.bind(*numero),
            Valor::Booleano(bandera) => consulta.bind(*bandera),
            Valor::Nulo => consulta.bind(Option::<String>::None),
        }
    }
}

pub fn consulta<'q>(sql: &'q str, valores: &[Valor]) -> Query<'q, Postgres, PgArguments> {
    let mut consulta = sqlx::query(sql);
    for valor in valores {
        consulta = valor.a_sql(consulta);
    }
    consulta
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
                "INT8" => json!(fila.try_get::<i64, _>(indice)?),
                "FLOAT4" => json!(fila.try_get::<f32, _>(indice)?),
                "FLOAT8" => json!(fila.try_get::<f64, _>(indice)?),
                "TIMESTAMPTZ" => json!(iso_ms(fila.try_get::<DateTime<Utc>, _>(indice)?)),
                "TIMESTAMP" => {
                    let fecha = fila.try_get::<NaiveDateTime, _>(indice)?;
                    json!(iso_ms(DateTime::<Utc>::from_naive_utc_and_offset(
                        fecha, Utc
                    )))
                }
                "DATE" => json!(fecha_local_iso(fila.try_get::<NaiveDate, _>(indice)?)),
                "JSON" | "JSONB" => fila.try_get::<Value, _>(indice)?,
                _ => Value::Null,
            }
        };
        objeto.insert(nombre, valor);
    }
    Ok(Value::Object(objeto))
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

    #[test]
    fn debe_clasificar_las_columnas_del_esquema_field() {
        assert_eq!(tipo_columna("tenant_id"), TipoColumna::Uuid);
        assert_eq!(tipo_columna("fecha_inicio"), TipoColumna::Timestamptz);
        assert_eq!(tipo_columna("vencimiento_seguro"), TipoColumna::Fecha);
        assert_eq!(tipo_columna("personal_asignado_ids"), TipoColumna::Jsonb);
        assert_eq!(tipo_columna("completada"), TipoColumna::Booleano);
        assert_eq!(tipo_columna("anio"), TipoColumna::Entero);
        assert_eq!(tipo_columna("lat"), TipoColumna::Flotante);
        assert_eq!(tipo_columna("nombre"), TipoColumna::Texto);
    }

    #[test]
    fn debe_construir_valores_desde_json() {
        assert_eq!(
            Valor::desde_json(&json!("texto"), TipoColumna::Texto),
            Valor::Texto("texto".into())
        );
        assert_eq!(
            Valor::desde_json(&json!(12), TipoColumna::Entero),
            Valor::Entero(12)
        );
        assert_eq!(
            Valor::desde_json(&json!(12.5), TipoColumna::Flotante),
            Valor::Flotante(12.5)
        );
        assert_eq!(
            Valor::desde_json(&json!(true), TipoColumna::Booleano),
            Valor::Booleano(true)
        );
        assert_eq!(
            Valor::desde_json(&json!(null), TipoColumna::Texto),
            Valor::Nulo
        );
        assert_eq!(
            Valor::desde_json(&json!(["a"]), TipoColumna::Jsonb),
            Valor::Texto("[\"a\"]".into())
        );
    }
}
