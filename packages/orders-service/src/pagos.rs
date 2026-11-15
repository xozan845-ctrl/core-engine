//! Métodos de pago configurables y registro de pagos por orden.
//!
//! El catálogo `orders.metodos_pago` es global y lo administra un admin. El
//! registro `orders.pagos` guarda intentos/confirmaciones; la confirmación
//! manual (efectivo/transferencia) mueve la orden a `pagada`.

use crate::db::Db;
use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetodoPago {
    pub codigo: String,
    pub nombre: String,
    pub requiere_confirmacion: bool,
    pub activo: bool,
    pub config: Value,
}

pub struct Pagos {
    db: Arc<Db>,
}

impl Pagos {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar_metodos(&self) -> Result<Vec<MetodoPago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, requiere_confirmacion, activo, config
               FROM orders.metodos_pago ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_metodo).collect()
    }

    /// Método activo por código; `None` si no existe o está inactivo.
    pub async fn metodo_activo(&self, codigo: &str) -> Result<Option<MetodoPago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT codigo, nombre, requiere_confirmacion, activo, config
               FROM orders.metodos_pago
              WHERE codigo = $1 AND activo = true",
        )
        .bind(codigo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_metodo).transpose()
    }

    /// Registra el pago de una orden. Los métodos automáticos nacen
    /// confirmados; los manuales quedan pendientes hasta la confirmación.
    pub async fn registrar(
        &self,
        order_id: &str,
        empresa_id: Option<&str>,
        metodo: &MetodoPago,
        monto_cents: i64,
        moneda: &str,
    ) -> Result<(), ApiError> {
        let estado = if metodo.requiere_confirmacion {
            "pendiente"
        } else {
            "confirmado"
        };
        let mut tx = self
            .db
            .transaccion_servicio()
            .await
            .map_err(crate::event_store::error_db)?;
        sqlx::query(
            "INSERT INTO orders.pagos
               (order_id, empresa_id, metodo_codigo, estado, monto_cents, moneda,
                confirmado_en)
             VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6,
                     CASE WHEN $4 = 'confirmado' THEN NOW() ELSE NULL END)",
        )
        .bind(order_id)
        .bind(empresa_id)
        .bind(&metodo.codigo)
        .bind(estado)
        .bind(monto_cents)
        .bind(moneda)
        .execute(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        Ok(())
    }

    pub async fn crear_metodo(&self, datos: &Value) -> Result<MetodoPago, ApiError> {
        let codigo = validar_codigo(campo(datos, "codigo"))?;
        let nombre = validar_nombre(campo(datos, "nombre"))?;
        let requiere_confirmacion = datos
            .get("requiere_confirmacion")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let activo = datos.get("activo").and_then(Value::as_bool).unwrap_or(true);
        let config = config_de(datos)?;
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "INSERT INTO orders.metodos_pago
               (codigo, nombre, requiere_confirmacion, activo, config)
             VALUES ($1, $2, $3, $4, $5::jsonb)
             RETURNING codigo, nombre, requiere_confirmacion, activo, config",
        )
        .bind(&codigo)
        .bind(&nombre)
        .bind(requiere_confirmacion)
        .bind(activo)
        .bind(&config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear el metodo de pago."))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_metodo(&fila)
    }

    pub async fn actualizar_metodo(
        &self,
        codigo: &str,
        datos: &Value,
    ) -> Result<MetodoPago, ApiError> {
        let codigo = validar_codigo(codigo)?;
        let nombre = match datos.get("nombre") {
            Some(valor) => Some(validar_nombre(valor.as_str().unwrap_or_default())?),
            None => None,
        };
        let requiere_confirmacion = datos.get("requiere_confirmacion").and_then(Value::as_bool);
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
            "UPDATE orders.metodos_pago
                SET nombre = COALESCE($2, nombre),
                    requiere_confirmacion = COALESCE($3, requiere_confirmacion),
                    activo = COALESCE($4, activo),
                    config = COALESCE($5::jsonb, config),
                    actualizado_en = NOW()
              WHERE codigo = $1
              RETURNING codigo, nombre, requiere_confirmacion, activo, config",
        )
        .bind(&codigo)
        .bind(nombre)
        .bind(requiere_confirmacion)
        .bind(activo)
        .bind(config)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Metodo de pago", &codigo))?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_metodo(&fila)
    }
}

fn fila_metodo(fila: &sqlx::postgres::PgRow) -> Result<MetodoPago, ApiError> {
    Ok(MetodoPago {
        codigo: fila
            .try_get("codigo")
            .map_err(crate::event_store::error_db)?,
        nombre: fila
            .try_get("nombre")
            .map_err(crate::event_store::error_db)?,
        requiere_confirmacion: fila
            .try_get("requiere_confirmacion")
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
            return ApiError::conflicto("El metodo de pago ya existe.");
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
            validar_codigo("Transferencia").expect("codigo"),
            "transferencia"
        );
        assert!(validar_codigo("1malo").is_err());
        assert!(validar_nombre("Efectivo").is_ok());
        assert!(validar_nombre("x").is_err());
    }

    #[test]
    fn debe_exigir_config_objeto() {
        assert_eq!(config_de(&json!({})).expect("config"), json!({}));
        assert!(config_de(&json!({"config": []})).is_err());
    }
}
