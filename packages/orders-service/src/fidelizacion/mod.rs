//! Fidelización (capacidad `fidelizacion`): programas, tarjetas, movimientos,
//! recompensas y canjes.
//!
//! El programa es único por empresa. Las tarjetas se emiten por parte
//! (`identity.partes`) y los movimientos registran acumulaciones, canjes,
//! ajustes y expiraciones. La acumulación por orden es idempotente gracias al
//! índice único `(tarjeta_id, order_id)` de los movimientos de acumulación.

mod comprador;
mod programa;
mod recompensas;
mod tarjetas;

mod prelude {
    pub(crate) use crate::db::{jsonb, timestamp, uuid_opcional, uuid_texto, Db};
    pub(crate) use crate::errors::ApiError;
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::{json, Value};
    pub(crate) use sqlx::Row;
    pub(crate) use std::sync::Arc;
}
use self::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Programa {
    pub id: String,
    pub empresa_id: String,
    pub nombre: String,
    pub activo: bool,
    /// Puntos otorgados por cada unidad de moneda (100 centavos).
    pub puntos_por_unidad: i32,
    pub expira_meses: Option<i32>,
    pub config: Value,
    pub creado_en: String,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tarjeta {
    pub id: String,
    pub empresa_id: String,
    pub programa_id: String,
    pub parte_id: String,
    pub parte_nombre: String,
    pub codigo: String,
    pub puntos: i32,
    pub estado: String,
    pub creado_en: String,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Movimiento {
    pub id: String,
    pub empresa_id: String,
    pub tarjeta_id: String,
    pub tipo: String,
    pub puntos: i32,
    pub saldo_tras: i32,
    pub order_id: Option<String>,
    pub event_id: Option<String>,
    pub descripcion: String,
    pub creado_por: Option<String>,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recompensa {
    pub id: String,
    pub empresa_id: String,
    pub codigo: String,
    pub nombre: String,
    pub descripcion: String,
    pub puntos: i32,
    pub stock: Option<i32>,
    pub activo: bool,
    pub config: Value,
    pub creado_en: String,
    pub actualizado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Canje {
    pub id: String,
    pub empresa_id: String,
    pub tarjeta_id: String,
    pub recompensa_id: String,
    pub movimiento_id: String,
    pub order_id: Option<String>,
    pub estado: String,
    pub puntos: i32,
    pub creado_por: Option<String>,
    pub creado_en: String,
}

pub struct Fidelizacion {
    db: Arc<Db>,
}

const COLUMNAS_PROGRAMA: &str =
    "id, empresa_id, nombre, activo, puntos_por_unidad, expira_meses, config,
     creado_en, actualizado_en";

const COLUMNAS_TARJETA: &str =
    "t.id, t.empresa_id, t.programa_id, t.parte_id, p.nombre AS parte_nombre,
     t.codigo, t.puntos, t.estado, t.creado_en, t.actualizado_en";

const COLUMNAS_MOVIMIENTO: &str =
    "id, empresa_id, tarjeta_id, tipo, puntos, saldo_tras, order_id, event_id,
     descripcion, creado_por, creado_en";

const COLUMNAS_RECOMPENSA: &str =
    "id, empresa_id, codigo, nombre, descripcion, puntos, stock, activo, config,
     creado_en, actualizado_en";

const COLUMNAS_CANJE: &str =
    "id, empresa_id, tarjeta_id, recompensa_id, movimiento_id, order_id, estado,
     puntos, creado_por, creado_en";

impl Fidelizacion {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }
}

fn fila_programa(fila: &sqlx::postgres::PgRow) -> Result<Programa, ApiError> {
    Ok(Programa {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        nombre: fila.try_get("nombre").map_err(error_db)?,
        activo: fila.try_get("activo").map_err(error_db)?,
        puntos_por_unidad: fila.try_get("puntos_por_unidad").map_err(error_db)?,
        expira_meses: fila.try_get("expira_meses").map_err(error_db)?,
        config: jsonb(fila, "config").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
    })
}

fn fila_tarjeta(fila: &sqlx::postgres::PgRow) -> Result<Tarjeta, ApiError> {
    Ok(Tarjeta {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        programa_id: uuid_texto(fila, "programa_id").map_err(error_db)?,
        parte_id: uuid_texto(fila, "parte_id").map_err(error_db)?,
        parte_nombre: fila.try_get("parte_nombre").map_err(error_db)?,
        codigo: fila.try_get("codigo").map_err(error_db)?,
        puntos: fila.try_get("puntos").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
    })
}

fn fila_movimiento(fila: &sqlx::postgres::PgRow) -> Result<Movimiento, ApiError> {
    Ok(Movimiento {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        tarjeta_id: uuid_texto(fila, "tarjeta_id").map_err(error_db)?,
        tipo: fila.try_get("tipo").map_err(error_db)?,
        puntos: fila.try_get("puntos").map_err(error_db)?,
        saldo_tras: fila.try_get("saldo_tras").map_err(error_db)?,
        order_id: uuid_opcional(fila, "order_id").map_err(error_db)?,
        event_id: uuid_opcional(fila, "event_id").map_err(error_db)?,
        descripcion: fila.try_get("descripcion").map_err(error_db)?,
        creado_por: uuid_opcional(fila, "creado_por").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
    })
}

fn fila_recompensa(fila: &sqlx::postgres::PgRow) -> Result<Recompensa, ApiError> {
    Ok(Recompensa {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        codigo: fila.try_get("codigo").map_err(error_db)?,
        nombre: fila.try_get("nombre").map_err(error_db)?,
        descripcion: fila.try_get("descripcion").map_err(error_db)?,
        puntos: fila.try_get("puntos").map_err(error_db)?,
        stock: fila.try_get("stock").map_err(error_db)?,
        activo: fila.try_get("activo").map_err(error_db)?,
        config: jsonb(fila, "config").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        actualizado_en: timestamp(fila, "actualizado_en").map_err(error_db)?,
    })
}

fn fila_canje(fila: &sqlx::postgres::PgRow) -> Result<Canje, ApiError> {
    Ok(Canje {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        empresa_id: uuid_texto(fila, "empresa_id").map_err(error_db)?,
        tarjeta_id: uuid_texto(fila, "tarjeta_id").map_err(error_db)?,
        recompensa_id: uuid_texto(fila, "recompensa_id").map_err(error_db)?,
        movimiento_id: uuid_texto(fila, "movimiento_id").map_err(error_db)?,
        order_id: uuid_opcional(fila, "order_id").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        puntos: fila.try_get("puntos").map_err(error_db)?,
        creado_por: uuid_opcional(fila, "creado_por").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
    })
}

fn texto(dto: &Value, campo: &str) -> String {
    dto.get(campo)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string()
}

fn entero(dto: &Value, campo: &str) -> Option<i32> {
    dto.get(campo)
        .and_then(Value::as_i64)
        .and_then(|valor| i32::try_from(valor).ok())
}

fn codigo_valido(codigo: &str) -> bool {
    let bytes = codigo.as_bytes();
    (4..=32).contains(&bytes.len())
        && bytes[0].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b'-')
}

fn codigo_recompensa_valido(codigo: &str) -> bool {
    let bytes = codigo.as_bytes();
    (2..=40).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
        })
}

fn es_unico(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|codigo| codigo == "23505")
}

fn error_db(error: sqlx::Error) -> ApiError {
    crate::event_store::error_db(error)
}
