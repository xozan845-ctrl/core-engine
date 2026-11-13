//! `OrderEventStore`: historial append-only de eventos (fuente de verdad).
//!
//! Réplica de `order-event-store.ts`: versión optimista por agregado con
//! reintentos ante `23505` y `SAVEPOINT` cuando participa de una transacción.

use crate::db::{iso_ms, uuid_texto};
use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use uuid::Uuid;

const INTENTOS_VERSION: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EventoDeOrden {
    pub event_id: String,
    pub aggregate_id: String,
    pub aggregate_type: String,
    pub tipo: String,
    pub payload: Value,
    pub metadata: Value,
    pub version: i32,
    pub creado_en: String,
}

pub struct OrderEventStore;

impl OrderEventStore {
    pub async fn append(
        &self,
        db: &crate::db::Db,
        order_id: &str,
        tipo: &str,
        payload: &Value,
    ) -> Result<EventoDeOrden, ApiError> {
        for intento in 0..INTENTOS_VERSION {
            let resultado = sqlx::query(
                "INSERT INTO orders.eventos (aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en)
                 SELECT $1, 'Order', $2, $3::jsonb, '{}'::jsonb, COALESCE(MAX(version), 0) + 1, NOW()
                 FROM orders.eventos WHERE aggregate_id = $1
                 RETURNING event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en",
            )
            .bind(parsear_uuid(order_id)?)
            .bind(tipo)
            .bind(sqlx::types::Json(payload))
            .fetch_one(db.pool())
            .await;

            match resultado {
                Ok(fila) => return evento_de_fila(&fila),
                Err(error) if es_violacion_unicidad(&error) && intento < INTENTOS_VERSION - 1 => {}
                Err(error) => return Err(error_db(error)),
            }
        }
        Err(ApiError::interno())
    }

    pub async fn append_en_transaccion(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        order_id: &str,
        tipo: &str,
        payload: &Value,
    ) -> Result<EventoDeOrden, ApiError> {
        for intento in 0..INTENTOS_VERSION {
            sqlx::query("SAVEPOINT eventos_append")
                .execute(&mut **tx)
                .await
                .map_err(error_db)?;
            let resultado = sqlx::query(
                "INSERT INTO orders.eventos (aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en)
                 SELECT $1, 'Order', $2, $3::jsonb, '{}'::jsonb, COALESCE(MAX(version), 0) + 1, NOW()
                 FROM orders.eventos WHERE aggregate_id = $1
                 RETURNING event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en",
            )
            .bind(parsear_uuid(order_id)?)
            .bind(tipo)
            .bind(sqlx::types::Json(payload))
            .fetch_one(&mut **tx)
            .await;

            match resultado {
                Ok(fila) => {
                    sqlx::query("RELEASE SAVEPOINT eventos_append")
                        .execute(&mut **tx)
                        .await
                        .map_err(error_db)?;
                    return evento_de_fila(&fila);
                }
                Err(error) if es_violacion_unicidad(&error) && intento < INTENTOS_VERSION - 1 => {
                    sqlx::query("ROLLBACK TO SAVEPOINT eventos_append")
                        .execute(&mut **tx)
                        .await
                        .map_err(error_db)?;
                }
                Err(error) => {
                    let _ = sqlx::query("ROLLBACK TO SAVEPOINT eventos_append")
                        .execute(&mut **tx)
                        .await;
                    return Err(error_db(error));
                }
            }
        }
        Err(ApiError::interno())
    }

    pub async fn historia_de(
        &self,
        db: &crate::db::Db,
        order_id: &str,
    ) -> Result<Vec<EventoDeOrden>, ApiError> {
        let order_uuid = parsear_uuid(order_id)?;
        let filas = sqlx::query(
            "SELECT event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en
             FROM orders.eventos WHERE aggregate_id = $1 ORDER BY version ASC",
        )
        .bind(order_uuid)
        .fetch_all(db.pool())
        .await
        .map_err(error_db)?;
        filas
            .iter()
            .map(evento_de_fila)
            .collect::<Result<Vec<_>, _>>()
    }

    pub async fn ultimo_evento_de(
        &self,
        db: &crate::db::Db,
        order_id: &str,
    ) -> Result<Option<EventoDeOrden>, ApiError> {
        let order_uuid = parsear_uuid(order_id)?;
        let fila = sqlx::query(
            "SELECT event_id, aggregate_id, aggregate_type, tipo, payload, metadata, version, creado_en
             FROM orders.eventos WHERE aggregate_id = $1 ORDER BY version DESC LIMIT 1",
        )
        .bind(order_uuid)
        .fetch_optional(db.pool())
        .await
        .map_err(error_db)?;
        fila.as_ref().map(evento_de_fila).transpose()
    }
}

pub fn parsear_uuid(valor: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(valor)
        .map_err(|_| ApiError::dominio("ID_INVALIDO", "El identificador debe ser un UUID."))
}

pub fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    matches!(
        error,
        sqlx::Error::Database(base) if base.code().as_deref() == Some("23505")
    )
}

pub fn error_db(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "db",
        crate::campos! { "msg": "Error de PostgreSQL", "err": error.to_string() },
    );
    ApiError::interno()
}

fn evento_de_fila(fila: &sqlx::postgres::PgRow) -> Result<EventoDeOrden, ApiError> {
    Ok(EventoDeOrden {
        event_id: uuid_texto(fila, "event_id").map_err(error_db)?,
        aggregate_id: uuid_texto(fila, "aggregate_id").map_err(error_db)?,
        aggregate_type: fila.try_get("aggregate_type").map_err(error_db)?,
        tipo: fila.try_get("tipo").map_err(error_db)?,
        payload: fila.try_get("payload").map_err(error_db)?,
        metadata: fila.try_get("metadata").map_err(error_db)?,
        version: fila.try_get("version").map_err(error_db)?,
        creado_en: iso_ms(fila.try_get("creado_en").map_err(error_db)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_rechazar_un_order_id_que_no_es_uuid() {
        let error = parsear_uuid("no-uuid").expect_err("invalido");
        assert_eq!(error.codigo, "ID_INVALIDO");
    }

    #[test]
    fn debe_reconocer_una_violacion_de_unicidad() {
        // No se puede construir un `DatabaseError` real sin conexión; se
        // verifica que otros errores no se confundan con 23505.
        assert!(!es_violacion_unicidad(&sqlx::Error::RowNotFound));
    }
}
