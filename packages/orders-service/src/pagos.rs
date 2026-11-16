//! Métodos de pago configurables y registro de pagos por orden.
//!
//! El catálogo `orders.metodos_pago` es global y lo administra un admin. El
//! registro `orders.pagos` guarda intentos/confirmaciones; la confirmación
//! manual (efectivo/transferencia) mueve la orden a `pagada`.

use crate::db::{iso_ms, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use chrono::{DateTime, Utc};
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Pago {
    pub id: String,
    pub order_id: String,
    pub empresa_id: Option<String>,
    pub metodo_codigo: String,
    pub estado: String,
    pub monto_cents: i64,
    pub moneda: String,
    pub referencia: String,
    pub motivo_rechazo: String,
    pub confirmado_por: Option<String>,
    pub confirmado_en: Option<String>,
    pub creado_en: String,
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

    pub async fn listar_por_orden(&self, order_id: &str) -> Result<Vec<Pago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let filas = sqlx::query(
            "SELECT id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                    moneda, referencia, motivo_rechazo, confirmado_por,
                    confirmado_en, creado_en
               FROM orders.pagos
              WHERE order_id = $1::uuid
              ORDER BY creado_en DESC",
        )
        .bind(order_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        filas.iter().map(fila_pago).collect()
    }

    pub async fn obtener(&self, pago_id: &str) -> Result<Option<Pago>, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "SELECT id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                    moneda, referencia, motivo_rechazo, confirmado_por,
                    confirmado_en, creado_en
               FROM orders.pagos WHERE id = $1::uuid",
        )
        .bind(pago_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila.as_ref().map(fila_pago).transpose()
    }

    pub async fn confirmar(&self, pago_id: &str, actor_id: &str) -> Result<Pago, ApiError> {
        self.procesar(pago_id, "confirmado", "", actor_id).await
    }

    pub async fn rechazar(
        &self,
        pago_id: &str,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        self.procesar(pago_id, "rechazado", motivo, actor_id).await
    }

    async fn procesar(
        &self,
        pago_id: &str,
        estado: &str,
        motivo: &str,
        actor_id: &str,
    ) -> Result<Pago, ApiError> {
        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(crate::event_store::error_db)?;
        let fila = sqlx::query(
            "UPDATE orders.pagos
                SET estado = $2,
                    motivo_rechazo = CASE WHEN $2 = 'rechazado' THEN $3 ELSE motivo_rechazo END,
                    confirmado_por = $4::uuid,
                    confirmado_en = NOW(),
                    actualizado_en = NOW()
              WHERE id = $1::uuid AND estado = 'pendiente'
              RETURNING id, order_id, empresa_id, metodo_codigo, estado, monto_cents,
                        moneda, referencia, motivo_rechazo, confirmado_por,
                        confirmado_en, creado_en",
        )
        .bind(pago_id)
        .bind(estado)
        .bind(motivo)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(crate::event_store::error_db)?;
        let Some(fila) = fila else {
            let existe: Option<String> =
                sqlx::query_scalar("SELECT estado FROM orders.pagos WHERE id = $1::uuid")
                    .bind(pago_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(crate::event_store::error_db)?;
            return match existe {
                Some(estado) => Err(ApiError::conflicto(format!(
                    "El pago ya fue procesado (estado {estado})."
                ))),
                None => Err(ApiError::no_encontrado("Pago", pago_id)),
            };
        };
        tx.commit().await.map_err(crate::event_store::error_db)?;
        fila_pago(&fila)
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

fn fila_pago(fila: &sqlx::postgres::PgRow) -> Result<Pago, ApiError> {
    let confirmado_en: Option<DateTime<Utc>> = fila
        .try_get("confirmado_en")
        .map_err(crate::event_store::error_db)?;
    let creado_en: DateTime<Utc> = fila
        .try_get("creado_en")
        .map_err(crate::event_store::error_db)?;
    Ok(Pago {
        id: uuid_texto(fila, "id").map_err(crate::event_store::error_db)?,
        order_id: uuid_texto(fila, "order_id").map_err(crate::event_store::error_db)?,
        empresa_id: uuid_opcional(fila, "empresa_id").map_err(crate::event_store::error_db)?,
        metodo_codigo: fila
            .try_get("metodo_codigo")
            .map_err(crate::event_store::error_db)?,
        estado: fila
            .try_get("estado")
            .map_err(crate::event_store::error_db)?,
        monto_cents: i64::from(
            fila.try_get::<i32, _>("monto_cents")
                .map_err(crate::event_store::error_db)?,
        ),
        moneda: fila
            .try_get("moneda")
            .map_err(crate::event_store::error_db)?,
        referencia: fila
            .try_get("referencia")
            .map_err(crate::event_store::error_db)?,
        motivo_rechazo: fila
            .try_get("motivo_rechazo")
            .map_err(crate::event_store::error_db)?,
        confirmado_por: uuid_opcional(fila, "confirmado_por")
            .map_err(crate::event_store::error_db)?,
        confirmado_en: confirmado_en.map(iso_ms),
        creado_en: iso_ms(creado_en),
    })
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
