//! `OrderView`: proyección de lectura del lado CQRS (Tabla 8).
//!
//! Lee de `orders.orden_vista` (poblada por trigger desde el event store).
//! Réplica de `order-view.repository.ts`.

use crate::db::iso_ms;
use crate::errors::ApiError;
use crate::event_store::error_db;
use crate::order_state::ItemOrden;
use serde::Serialize;
use serde_json::Value;
use sqlx::Row;

const COLUMNAS: &str = "id, vendedor_id, comprador_id, parte_id, tienda_id, estado, total_cents, comision_cents, moneda,
              items_json, direccion_envio, creado_en, actualizado_en,
              tipo_orden, metodo_pago, fulfillment_config,
              pagada_en, enviada_en, entregada_en, cancelada_en, devuelta_en";

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OrderView {
    pub id: String,
    pub vendedor_id: String,
    pub comprador_id: String,
    pub parte_id: Option<String>,
    pub tienda_id: String,
    pub estado: String,
    pub total_cents: i64,
    pub comision_cents: i64,
    pub moneda: String,
    pub items: Vec<ItemOrden>,
    pub direccion_envio: Value,
    pub creado_en: String,
    pub actualizado_en: String,
    pub tipo_orden: String,
    pub metodo_pago: String,
    pub fulfillment_config: Value,
    pub pagada_en: Option<String>,
    pub enviada_en: Option<String>,
    pub entregada_en: Option<String>,
    pub cancelada_en: Option<String>,
    pub devuelta_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TimelineEvento {
    pub id: i64,
    pub tipo: String,
    pub payload: Value,
    pub version: i32,
    pub creado_en: String,
}

pub struct OrderViewRepository;

impl OrderViewRepository {
    pub async fn encontrar(
        &self,
        db: &crate::db::Db,
        id: &str,
    ) -> Result<Option<OrderView>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM orders.orden_vista WHERE id = $1"
        ))
        .bind(crate::event_store::parsear_uuid(id)?)
        .fetch_optional(db.pool())
        .await
        .map_err(error_db)?;
        fila.as_ref().map(serializar).transpose()
    }

    pub async fn listar_de_cliente(
        &self,
        db: &crate::db::Db,
        cliente_id: &str,
        estado: Option<&str>,
    ) -> Result<Vec<OrderView>, ApiError> {
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM orders.orden_vista
             WHERE comprador_id = $1 AND ($2::text IS NULL OR estado = $2)
             ORDER BY creado_en DESC LIMIT 100"
        ))
        .bind(crate::event_store::parsear_uuid(cliente_id)?)
        .bind(estado)
        .fetch_all(db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(serializar).collect()
    }

    pub async fn listar_de_vendedor(
        &self,
        db: &crate::db::Db,
        vendedor_id: &str,
        estado: Option<&str>,
    ) -> Result<Vec<OrderView>, ApiError> {
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM orders.orden_vista
             WHERE EXISTS (SELECT 1 FROM jsonb_array_elements(items_json) item
                           WHERE item->>'vendedor_id' = $1)
               AND ($2::text IS NULL OR estado = $2)
             ORDER BY creado_en DESC LIMIT 100"
        ))
        .bind(vendedor_id)
        .bind(estado)
        .fetch_all(db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(serializar).collect()
    }

    pub async fn listar_de_tienda(
        &self,
        db: &crate::db::Db,
        tienda_id: &str,
        estado: Option<&str>,
        vendedor_id: Option<&str>,
        empresa_id: Option<&str>,
    ) -> Result<Vec<OrderView>, ApiError> {
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM orders.orden_vista
             WHERE EXISTS (SELECT 1 FROM jsonb_array_elements(items_json) item
                           WHERE item->>'tienda_id' = $1
                             AND ($2::text IS NULL OR item->>'empresa_id' = $2)
                             AND ($3::text IS NULL OR item->>'vendedor_id' = $3))
               AND ($4::text IS NULL OR estado = $4)
             ORDER BY creado_en DESC LIMIT 100"
        ))
        .bind(tienda_id)
        .bind(empresa_id)
        .bind(vendedor_id)
        .bind(estado)
        .fetch_all(db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(serializar).collect()
    }

    pub async fn listar_de_empresa(
        &self,
        db: &crate::db::Db,
        empresa_id: &str,
        estado: Option<&str>,
        tienda_id: Option<&str>,
        limite: Option<i64>,
    ) -> Result<Vec<OrderView>, ApiError> {
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM orders.orden_vista
             WHERE EXISTS (SELECT 1 FROM jsonb_array_elements(items_json) item
                           WHERE item->>'empresa_id' = $1
                             AND ($2::text IS NULL OR item->>'tienda_id' = $2))
               AND ($3::text IS NULL OR estado = $3)
             ORDER BY creado_en DESC LIMIT $4"
        ))
        .bind(empresa_id)
        .bind(tienda_id)
        .bind(estado)
        .bind(limite.unwrap_or(100))
        .fetch_all(db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(serializar).collect()
    }

    pub async fn listar_todo(
        &self,
        db: &crate::db::Db,
        estado: Option<&str>,
        limite: Option<i64>,
    ) -> Result<Vec<OrderView>, ApiError> {
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM orders.orden_vista
             WHERE ($1::text IS NULL OR estado = $1)
             ORDER BY creado_en DESC LIMIT $2"
        ))
        .bind(estado)
        .bind(limite.unwrap_or(200))
        .fetch_all(db.pool())
        .await
        .map_err(error_db)?;
        filas.iter().map(serializar).collect()
    }

    pub async fn obtener_timeline(
        &self,
        db: &crate::db::Db,
        order_id: &str,
    ) -> Result<Vec<TimelineEvento>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, tipo, payload, version, creado_en
             FROM orders.orden_timeline WHERE order_id = $1 ORDER BY version",
        )
        .bind(crate::event_store::parsear_uuid(order_id)?)
        .fetch_all(db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas
            .iter()
            .map(|fila| TimelineEvento {
                id: fila.try_get("id").unwrap_or_default(),
                tipo: fila.try_get("tipo").unwrap_or_default(),
                payload: fila.try_get("payload").unwrap_or(Value::Null),
                version: fila.try_get("version").unwrap_or_default(),
                creado_en: fila.try_get("creado_en").map(iso_ms).unwrap_or_default(),
            })
            .collect())
    }
}

pub fn serializar(fila: &sqlx::postgres::PgRow) -> Result<OrderView, ApiError> {
    let items_json: Value = fila.try_get("items_json").map_err(error_db)?;
    let items: Vec<ItemOrden> = serde_json::from_value(items_json).unwrap_or_default();
    Ok(OrderView {
        id: crate::db::uuid_texto(fila, "id").map_err(error_db)?,
        vendedor_id: crate::db::uuid_texto(fila, "vendedor_id").map_err(error_db)?,
        comprador_id: crate::db::uuid_texto(fila, "comprador_id").map_err(error_db)?,
        parte_id: crate::db::uuid_opcional(fila, "parte_id").map_err(error_db)?,
        tienda_id: crate::db::uuid_texto(fila, "tienda_id").map_err(error_db)?,
        estado: fila.try_get("estado").map_err(error_db)?,
        total_cents: i64::from(fila.try_get::<i32, _>("total_cents").map_err(error_db)?),
        comision_cents: i64::from(fila.try_get::<i32, _>("comision_cents").map_err(error_db)?),
        moneda: fila.try_get("moneda").map_err(error_db)?,
        items,
        direccion_envio: fila
            .try_get::<Option<Value>, _>("direccion_envio")
            .map_err(error_db)?
            .unwrap_or(Value::Null),
        creado_en: fila.try_get("creado_en").map(iso_ms).map_err(error_db)?,
        actualizado_en: fila
            .try_get("actualizado_en")
            .map(iso_ms)
            .map_err(error_db)?,
        tipo_orden: fila.try_get("tipo_orden").map_err(error_db)?,
        metodo_pago: fila.try_get("metodo_pago").map_err(error_db)?,
        fulfillment_config: fila
            .try_get("fulfillment_config")
            .unwrap_or(Value::Object(Default::default())),
        pagada_en: fila.try_get("pagada_en").ok().map(iso_ms),
        enviada_en: fila.try_get("enviada_en").ok().map(iso_ms),
        entregada_en: fila.try_get("entregada_en").ok().map(iso_ms),
        cancelada_en: fila.try_get("cancelada_en").ok().map(iso_ms),
        devuelta_en: fila.try_get("devuelta_en").ok().map(iso_ms),
    })
}

pub fn es_uuid(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    valor.len() == 36
        && bytes.iter().enumerate().all(|(indice, byte)| match indice {
            8 | 13 | 18 | 23 => *byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_reconocer_uuids_canonicos() {
        assert!(es_uuid("123e4567-e89b-12d3-a456-426614174000"));
        assert!(!es_uuid("123e4567-e89b-12d3-a456"));
        assert!(!es_uuid("123e4567-e89b-12d3-a456-42661417400z"));
    }

    #[test]
    fn debe_serializar_una_vista_de_prueba() {
        let vista = OrderView {
            id: "o-1".into(),
            vendedor_id: "v-1".into(),
            comprador_id: "c-1".into(),
            parte_id: None,
            tienda_id: "t-1".into(),
            estado: "creada".into(),
            total_cents: 100,
            comision_cents: 12,
            moneda: "NIO".into(),
            items: Vec::new(),
            direccion_envio: Value::Null,
            creado_en: "2026-01-01T00:00:00.000Z".into(),
            actualizado_en: "2026-01-01T00:00:00.000Z".into(),
            tipo_orden: "compra".into(),
            metodo_pago: "simulado".into(),
            fulfillment_config: Value::Object(Default::default()),
            pagada_en: None,
            enviada_en: None,
            entregada_en: None,
            cancelada_en: None,
            devuelta_en: None,
        };
        let json = serde_json::to_value(vista).expect("json");
        assert_eq!(json["parte_id"], Value::Null);
        assert_eq!(json["fulfillment_config"], serde_json::json!({}));
    }
}
