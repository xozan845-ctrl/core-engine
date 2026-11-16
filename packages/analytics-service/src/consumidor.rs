//! Consumidor de hechos: órdenes completadas y comisiones acreditadas.
//!
//! Idempotencia por `event_id`. La orden se consulta a la superficie interna de
//! `orders-service`; el evento de comisión aporta los montos financieros.

use crate::clients::Ordenes;
use crate::db::{error_db, Db};
use crate::errors::ApiError;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::rabbit::Handler;
use futures_util::future::BoxFuture;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use std::sync::Arc;

pub const COLA_ORDERS: &str = "analytics.orders";
pub const COLA_COMISIONES: &str = "analytics.comisiones";
pub const EVENTO_ORDER_COMPLETADO: &str = "order.completado";
pub const EVENTO_COMISION_ACREDITADA: &str = "comision.acreditada";
pub const USUARIO_SISTEMA: &str = "00000000-0000-0000-0000-000000000000";

pub struct AnalyticsConsumer {
    db: Arc<Db>,
    ordenes: Arc<dyn Ordenes>,
}

impl AnalyticsConsumer {
    pub fn nuevo(db: Arc<Db>, ordenes: Arc<dyn Ordenes>) -> Self {
        Self { db, ordenes }
    }

    pub async fn manejar(&self, evento: Value) -> Result<(), ApiError> {
        let Some(tipo) = evento.get("tipo").and_then(Value::as_str) else {
            return Ok(());
        };
        if !matches!(tipo, EVENTO_ORDER_COMPLETADO | EVENTO_COMISION_ACREDITADA) {
            return Ok(());
        }
        let Some(event_id) = evento.get("event_id").and_then(Value::as_str) else {
            return Ok(());
        };
        let contexto = ContextoCorrelacion {
            request_id: format!("analytics-event-{event_id}"),
            servicio: "analytics-consumer".to_string(),
            usuario_id: Some(USUARIO_SISTEMA.to_string()),
            rol: Some("super_admin".to_string()),
            tenant_id: None,
            negocio_id: None,
        };
        ejecutar_con_contexto(contexto, async {
            match tipo {
                EVENTO_ORDER_COMPLETADO => self.orden_completada(&evento, event_id).await,
                EVENTO_COMISION_ACREDITADA => self.comision(&evento, event_id).await,
                _ => Ok(()),
            }
        })
        .await
    }

    async fn orden_completada(&self, evento: &Value, event_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        if self.ya_procesado_tx(&mut tx, event_id).await? {
            tx.commit().await.map_err(error_db)?;
            return Ok(());
        }
        let data = datos(evento)?;
        let order_id = texto(data, "order_id")?;
        let orden = self.ordenes.obtener(&order_id).await.ok_or_else(|| {
            ApiError::interno_mensaje("Orden no disponible para proyectar la venta.")
        })?;
        if orden.items.is_empty() {
            return Err(ApiError::interno_mensaje("La orden no tiene líneas."));
        }
        let comprador = orden.comprador_id.as_deref();
        let creado_en = orden.creado_en.as_deref();
        for item in &orden.items {
            let (Some(empresa_id), Some(tienda_id)) =
                (item.empresa_id.as_deref(), item.tienda_id.as_deref())
            else {
                return Err(ApiError::interno_mensaje(
                    "La línea no tiene empresa o tienda.",
                ));
            };
            let venta_cents = item
                .precio_unitario_cents
                .checked_mul(item.cantidad)
                .ok_or_else(|| ApiError::interno_mensaje("La línea desborda el monto."))?;
            sqlx::query(
                "INSERT INTO analytics.hechos_venta
                   (order_id, linea_clave, empresa_id, negocio_id, tienda_id,
                    vendedor_id, comprador_id, cantidad, precio_unitario_cents,
                    venta_cents, orden_creada_en)
                 VALUES ($1::uuid, $2, $3::uuid, $4::uuid, $5::uuid, $6::uuid,
                         $7::uuid, $8, $9, $10, COALESCE($11::timestamptz, NOW()))
                 ON CONFLICT (order_id, linea_clave) DO UPDATE
                   SET cantidad = EXCLUDED.cantidad,
                       precio_unitario_cents = EXCLUDED.precio_unitario_cents,
                       venta_cents = EXCLUDED.venta_cents,
                       comprador_id = EXCLUDED.comprador_id,
                       orden_creada_en = EXCLUDED.orden_creada_en",
            )
            .bind(&order_id)
            .bind(&item.oferta_id)
            .bind(empresa_id)
            .bind(item.negocio_id.as_deref())
            .bind(tienda_id)
            .bind(item.vendedor_id.as_deref())
            .bind(comprador)
            .bind(item.cantidad)
            .bind(item.precio_unitario_cents)
            .bind(venta_cents)
            .bind(creado_en)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;
        }
        self.marcar_procesado_tx(&mut tx, event_id, EVENTO_ORDER_COMPLETADO)
            .await?;
        tx.commit().await.map_err(error_db)?;
        crate::logs::info(
            "analytics.pedidos",
            crate::campos! {
                "msg": "Venta proyectada",
                "order_id": order_id,
                "lineas": orden.items.len(),
            },
        );
        Ok(())
    }

    async fn comision(&self, evento: &Value, event_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        if self.ya_procesado_tx(&mut tx, event_id).await? {
            tx.commit().await.map_err(error_db)?;
            return Ok(());
        }
        let data = datos(evento)?;
        let order_id = texto(data, "order_id")?;
        let linea = data
            .get("linea_clave")
            .and_then(Value::as_str)
            .or_else(|| data.get("oferta_id").and_then(Value::as_str))
            .unwrap_or(&order_id)
            .to_string();
        let comision_cents = entero(data, "monto_cents")?;
        let empresa_id = texto(data, "empresa_id")?;
        sqlx::query(
            "INSERT INTO analytics.comisiones
               (order_id, linea_clave, empresa_id, negocio_id, tienda_id,
                vendedor_id, comision_cents, monto_vendedor_cents,
                monto_empresario_cents, participacion_porcentaje)
             VALUES ($1::uuid, $2, $3::uuid, $4::uuid, $5::uuid, $6::uuid,
                     $7, $8, $9, $10)
             ON CONFLICT (order_id, linea_clave) DO UPDATE
               SET comision_cents = EXCLUDED.comision_cents,
                   monto_vendedor_cents = EXCLUDED.monto_vendedor_cents,
                   monto_empresario_cents = EXCLUDED.monto_empresario_cents,
                   participacion_porcentaje = EXCLUDED.participacion_porcentaje",
        )
        .bind(&order_id)
        .bind(&linea)
        .bind(&empresa_id)
        .bind(data.get("negocio_id").and_then(Value::as_str))
        .bind(data.get("tienda_id").and_then(Value::as_str))
        .bind(data.get("vendedor_id").and_then(Value::as_str))
        .bind(comision_cents)
        .bind(entero_opcional(data, "monto_vendedor_cents").unwrap_or(0))
        .bind(entero_opcional(data, "monto_empresario_cents").unwrap_or(0))
        .bind(data.get("participacion_porcentaje").and_then(Value::as_i64))
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;
        self.marcar_procesado_tx(&mut tx, event_id, EVENTO_COMISION_ACREDITADA)
            .await?;
        tx.commit().await.map_err(error_db)?;
        Ok(())
    }

    async fn ya_procesado_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
    ) -> Result<bool, ApiError> {
        let existe: Option<i32> = sqlx::query_scalar(
            "SELECT 1 FROM analytics.eventos_procesados WHERE event_id = $1::uuid",
        )
        .bind(event_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?;
        Ok(existe.is_some())
    }

    async fn marcar_procesado_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
        tipo: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO analytics.eventos_procesados (event_id, tipo)
             VALUES ($1::uuid, $2) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(event_id)
        .bind(tipo)
        .execute(&mut **tx)
        .await
        .map_err(error_db)?;
        Ok(())
    }
}

pub fn handler_consumidor(consumidor: Arc<AnalyticsConsumer>) -> Handler {
    Arc::new(move |payload| {
        let consumidor = Arc::clone(&consumidor);
        Box::pin(async move {
            consumidor
                .manejar(payload)
                .await
                .map_err(|error| error.to_string())
        }) as BoxFuture<'static, Result<(), String>>
    })
}

fn datos(evento: &Value) -> Result<&Value, ApiError> {
    evento
        .get("data")
        .ok_or_else(|| ApiError::interno_mensaje("Evento sin data."))
}

fn texto(data: &Value, clave: &str) -> Result<String, ApiError> {
    data.get(clave)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| ApiError::interno_mensaje(format!("Evento sin {clave}.")))
}

fn entero(data: &Value, clave: &str) -> Result<i64, ApiError> {
    data.get(clave)
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::interno_mensaje(format!("Evento sin {clave}.")))
}

fn entero_opcional(data: &Value, clave: &str) -> Option<i64> {
    data.get(clave).and_then(Value::as_i64)
}
