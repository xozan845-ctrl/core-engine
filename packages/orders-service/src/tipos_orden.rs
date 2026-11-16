//! Tipos de orden y transiciones configurables (Fase 3).
//!
//! La matriz vive en `orders.tipos_orden`/`orders.transiciones_orden`. El
//! código la consulta en cada transición, así un admin puede agregar tipos o
//! aristas sin desplegar.

use crate::db::Db;
use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TipoOrden {
    pub codigo: String,
    pub nombre: String,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransicionOrden {
    pub desde: String,
    pub hasta: String,
}

pub struct TiposOrden {
    db: Arc<Db>,
}

impl TiposOrden {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(&self) -> Result<Vec<TipoOrden>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, activo, config
               FROM orders.tipos_orden ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_tipo).collect()
    }

    pub async fn obtener(&self, codigo: &str) -> Result<Option<TipoOrden>, ApiError> {
        let codigo = codigo.trim().to_lowercase();
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT codigo, nombre, activo, config
               FROM orders.tipos_orden WHERE codigo = $1",
        )
        .bind(&codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_tipo).transpose()
    }

    pub async fn transiciones(&self, tipo: &str) -> Result<Vec<TransicionOrden>, ApiError> {
        let tipo = tipo.trim().to_lowercase();
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT desde, hasta FROM orders.transiciones_orden
              WHERE tipo_codigo = $1 ORDER BY desde, hasta",
        )
        .bind(&tipo)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas
            .iter()
            .map(|fila| {
                Ok(TransicionOrden {
                    desde: fila
                        .try_get("desde")
                        .map_err(crate::event_store::error_db)?,
                    hasta: fila
                        .try_get("hasta")
                        .map_err(crate::event_store::error_db)?,
                })
            })
            .collect()
    }

    /// Normaliza y exige que el tipo exista y esté activo.
    pub async fn validar_tipo(&self, codigo: &str) -> Result<String, ApiError> {
        let codigo = codigo.trim().to_lowercase();
        let tipo = self.obtener(&codigo).await?;
        match tipo {
            Some(tipo) if tipo.activo => Ok(tipo.codigo),
            _ => Err(ApiError::dominio(
                "TIPO_ORDEN_INVALIDO",
                "El tipo de orden no es valido.",
            )),
        }
    }

    /// Valida una transición contra la matriz configurada.
    pub async fn validar(&self, tipo: &str, desde: &str, hasta: &str) -> Result<(), ApiError> {
        let tipo = self.validar_tipo(tipo).await?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let existe = sqlx::query(
            "SELECT 1 FROM orders.transiciones_orden
              WHERE tipo_codigo = $1 AND desde = $2 AND hasta = $3",
        )
        .bind(&tipo)
        .bind(desde)
        .bind(hasta)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        if existe.is_some() {
            return Ok(());
        }
        Err(ApiError::dominio(
            "TRANSICION_INVALIDA",
            format!(
                "Transicion invalida de estado \"{desde}\" a \"{hasta}\" para orden \"{tipo}\"."
            ),
        ))
    }
    pub async fn detalle(&self, codigo: &str) -> Result<Value, ApiError> {
        let tipo = self
            .obtener(codigo)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Tipo de orden", codigo))?;
        let transiciones = self.transiciones(&tipo.codigo).await?;
        Ok(json!({ "tipo": tipo, "transiciones": transiciones }))
    }

    pub async fn crear(&self, datos: &Value) -> Result<TipoOrden, ApiError> {
        let codigo = validar_codigo(campo(datos, "codigo"))?;
        let nombre = validar_nombre(campo(datos, "nombre"))?;
        let config = config_de(datos)?;
        let activo = datos.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "INSERT INTO orders.tipos_orden (codigo, nombre, activo, config)
             VALUES ($1, $2, $3, $4::jsonb)
             RETURNING codigo, nombre, activo, config",
        )
        .bind(&codigo)
        .bind(&nombre)
        .bind(activo)
        .bind(&config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear el tipo de orden."))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_tipo(&fila)
    }

    pub async fn actualizar(&self, codigo: &str, datos: &Value) -> Result<TipoOrden, ApiError> {
        let codigo = codigo.trim().to_lowercase();
        let nombre = match datos.get("nombre") {
            Some(valor) => Some(validar_nombre(valor.as_str().unwrap_or_default())?),
            None => None,
        };
        let activo = datos.get("activo").and_then(Value::as_bool);
        let config = if datos.get("config").is_some() {
            Some(config_de(datos)?)
        } else {
            None
        };
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.tipos_orden
                SET nombre = COALESCE($2, nombre),
                    activo = COALESCE($3, activo),
                    config = COALESCE($4::jsonb, config),
                    actualizado_en = NOW()
              WHERE codigo = $1
              RETURNING codigo, nombre, activo, config",
        )
        .bind(&codigo)
        .bind(nombre)
        .bind(activo)
        .bind(config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Tipo de orden", &codigo))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_tipo(&fila)
    }

    pub async fn agregar_transicion(
        &self,
        tipo: &str,
        desde: &str,
        hasta: &str,
    ) -> Result<TransicionOrden, ApiError> {
        let tipo = self.validar_tipo(tipo).await?;
        let desde = desde.trim().to_lowercase();
        let hasta = hasta.trim().to_lowercase();
        if desde.is_empty() || hasta.is_empty() || desde == hasta {
            return Err(ApiError::validacion(json!([
                {"campo": "desde", "restricciones": ["isString"]},
                {"campo": "hasta", "restricciones": ["isString"]}
            ])));
        }
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let estados: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM orders.estados_orden
              WHERE codigo = ANY($1::text[])",
        )
        .bind(vec![desde.clone(), hasta.clone()])
        .fetch_one(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        if estados != 2 {
            return Err(ApiError::validacion(json!([
                {"campo": "desde", "restricciones": ["estadoExistente"]},
                {"campo": "hasta", "restricciones": ["estadoExistente"]}
            ])));
        }
        sqlx::query(
            "INSERT INTO orders.transiciones_orden (tipo_codigo, desde, hasta)
             VALUES ($1, $2, $3)",
        )
        .bind(&tipo)
        .bind(&desde)
        .bind(&hasta)
        .execute(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        Ok(TransicionOrden { desde, hasta })
    }

    pub async fn eliminar_transicion(
        &self,
        tipo: &str,
        desde: &str,
        hasta: &str,
    ) -> Result<(), ApiError> {
        let tipo = tipo.trim().to_lowercase();
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "DELETE FROM orders.transiciones_orden
              WHERE tipo_codigo = $1 AND desde = $2 AND hasta = $3",
        )
        .bind(&tipo)
        .bind(desde)
        .bind(hasta)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        if filas.rows_affected() == 0 {
            return Err(ApiError::no_encontrado("Transicion", &tipo));
        }
        Ok(())
    }
}

fn campo<'a>(datos: &'a Value, clave: &str) -> &'a str {
    datos.get(clave).and_then(Value::as_str).unwrap_or("")
}

fn validar_codigo(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    let mut chars = codigo.chars();
    let valido = matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&codigo.len())
        && codigo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valido {
        return Err(ApiError::validacion(json!([
            {"campo": "codigo", "restricciones": ["matches"]}
        ])));
    }
    Ok(codigo)
}

fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim();
    if !(2..=120).contains(&nombre.chars().count()) {
        return Err(ApiError::validacion(json!([
            {"campo": "nombre", "restricciones": ["isString"]}
        ])));
    }
    Ok(nombre.to_string())
}

fn config_de(datos: &Value) -> Result<Value, ApiError> {
    let config = datos.get("config").cloned().unwrap_or_else(|| json!({}));
    if !config.is_object() {
        return Err(ApiError::validacion(json!([
            {"campo": "config", "restricciones": ["isObject"]}
        ])));
    }
    Ok(config)
}

fn conflicto_o_interno(error: sqlx::Error) -> ApiError {
    if let Some(codigo) = error.as_database_error().and_then(|db| db.code()) {
        if codigo == "23505" {
            return ApiError::conflicto("El tipo o la transicion ya existe.");
        }
    }
    crate::event_store::error_db(error)
}

fn fila_tipo(fila: &sqlx::postgres::PgRow) -> Result<TipoOrden, ApiError> {
    Ok(TipoOrden {
        codigo: fila
            .try_get("codigo")
            .map_err(crate::event_store::error_db)?,
        nombre: fila
            .try_get("nombre")
            .map_err(crate::event_store::error_db)?,
        activo: fila
            .try_get("activo")
            .map_err(crate::event_store::error_db)?,
        config: fila
            .try_get("config")
            .map_err(crate::event_store::error_db)?,
    })
}
