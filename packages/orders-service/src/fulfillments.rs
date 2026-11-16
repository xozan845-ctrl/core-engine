//! Fulfillments configurables (Fase 4).
//!
//! El catálogo vive en `orders.fulfillments`. El checkout valida contra los
//! códigos activos en el siguiente PR.

use crate::db::Db;
use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fulfillment {
    pub codigo: String,
    pub nombre: String,
    pub requiere_direccion: bool,
    pub activo: bool,
    pub config: Value,
}

pub struct Fulfillments {
    db: Arc<Db>,
}

impl Fulfillments {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(&self) -> Result<Vec<Fulfillment>, ApiError> {
        self.consultar(None).await
    }

    pub async fn listar_activos(&self) -> Result<Vec<Fulfillment>, ApiError> {
        self.consultar(Some(true)).await
    }

    async fn consultar(&self, solo_activos: Option<bool>) -> Result<Vec<Fulfillment>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, requiere_direccion, activo, config
               FROM orders.fulfillments
              WHERE ($1::boolean IS NULL OR activo = $1)
              ORDER BY codigo",
        )
        .bind(solo_activos)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_fulfillment).collect()
    }

    pub async fn codigos_activos(&self) -> Result<Vec<String>, ApiError> {
        Ok(self
            .listar_activos()
            .await?
            .into_iter()
            .map(|fulfillment| fulfillment.codigo)
            .collect())
    }

    pub async fn validar(&self, codigo: &str) -> Result<String, ApiError> {
        let codigo = codigo.trim().to_lowercase();
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let existe =
            sqlx::query("SELECT 1 FROM orders.fulfillments WHERE codigo = $1 AND activo = true")
                .bind(&codigo)
                .fetch_optional(&mut *tx)
                .await
                .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        if existe.is_none() {
            return Err(ApiError::dominio(
                "FULFILLMENT_INVALIDO",
                "El fulfillment no es valido.",
            ));
        }
        Ok(codigo)
    }

    pub async fn crear(&self, datos: &Value) -> Result<Fulfillment, ApiError> {
        let codigo = validar_codigo(campo(datos, "codigo"))?;
        let nombre = validar_nombre(campo(datos, "nombre"))?;
        let requiere_direccion = datos
            .get("requiere_direccion")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let activo = datos.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let config = config_de(datos)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "INSERT INTO orders.fulfillments
               (codigo, nombre, requiere_direccion, activo, config)
             VALUES ($1, $2, $3, $4, $5::jsonb)
             RETURNING codigo, nombre, requiere_direccion, activo, config",
        )
        .bind(&codigo)
        .bind(&nombre)
        .bind(requiere_direccion)
        .bind(activo)
        .bind(&config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear el fulfillment."))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_fulfillment(&fila)
    }

    pub async fn actualizar(&self, codigo: &str, datos: &Value) -> Result<Fulfillment, ApiError> {
        let codigo = codigo.trim().to_lowercase();
        let nombre = match datos.get("nombre") {
            Some(valor) => Some(validar_nombre(valor.as_str().unwrap_or_default())?),
            None => None,
        };
        let requiere_direccion = datos.get("requiere_direccion").and_then(Value::as_bool);
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
            "UPDATE orders.fulfillments
                SET nombre = COALESCE($2, nombre),
                    requiere_direccion = COALESCE($3, requiere_direccion),
                    activo = COALESCE($4, activo),
                    config = COALESCE($5::jsonb, config),
                    actualizado_en = NOW()
              WHERE codigo = $1
              RETURNING codigo, nombre, requiere_direccion, activo, config",
        )
        .bind(&codigo)
        .bind(nombre)
        .bind(requiere_direccion)
        .bind(activo)
        .bind(config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Fulfillment", &codigo))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_fulfillment(&fila)
    }
}

fn fila_fulfillment(fila: &sqlx::postgres::PgRow) -> Result<Fulfillment, ApiError> {
    Ok(Fulfillment {
        codigo: fila
            .try_get("codigo")
            .map_err(crate::event_store::error_db)?,
        nombre: fila
            .try_get("nombre")
            .map_err(crate::event_store::error_db)?,
        requiere_direccion: fila
            .try_get("requiere_direccion")
            .map_err(crate::event_store::error_db)?,
        activo: fila
            .try_get("activo")
            .map_err(crate::event_store::error_db)?,
        config: fila
            .try_get("config")
            .map_err(crate::event_store::error_db)?,
    })
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
            return ApiError::conflicto("El fulfillment ya existe.");
        }
    }
    crate::event_store::error_db(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_codigos_y_nombres() {
        assert_eq!(
            validar_codigo("Retiro_Tienda").expect("codigo"),
            "retiro_tienda"
        );
        assert!(validar_codigo("1malo").is_err());
        assert!(validar_nombre("Retiro en tienda").is_ok());
        assert!(validar_nombre("x").is_err());
    }
}
