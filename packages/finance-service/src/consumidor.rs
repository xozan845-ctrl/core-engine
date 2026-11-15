//! Consumidor de eventos (tabla 22) que alimenta la contabilidad y el tablero:
//! - `order.created`       -> embudo: checkout completado
//! - `payment.procesado`   -> asiento Caja / Fondos por liquidar + métrica GMV
//! - `comision.acreditada` -> realización del ingreso (RN-04)
//! - `devolucion.solicitada` -> asiento de ajuste con la comisión revertida (RN-06)
//!
//! Idempotencia por `event_id` y persistencia atómica con el Outbox (ADR-03).

use crate::contabilidad::{Contabilidad, DetalleAsiento, ParametrosAsiento};
use crate::db::Db;
use crate::errors::{error_db, ApiError};
use crate::facturacion::Ordenes;
use crate::logs;
use crate::metricas::{DeltasVendedor, Metricas};
use crate::money;
use serde_json::Value;
use std::sync::Arc;

pub const ORDER_CREATED: &str = "order.created";
pub const PAYMENT_PROCESADO: &str = "payment.procesado";
pub const COMISION_ACREDITADA: &str = "comision.acreditada";
pub const DEVOLUCION_SOLICITADA: &str = "devolucion.solicitada";

pub struct Consumidor {
    db: Arc<Db>,
    contabilidad: Arc<Contabilidad>,
    ordenes: Arc<dyn Ordenes>,
    comision_tasa: f64,
}

impl Consumidor {
    pub fn nuevo(
        db: Arc<Db>,
        contabilidad: Arc<Contabilidad>,
        ordenes: Arc<dyn Ordenes>,
        comision_tasa: f64,
    ) -> Self {
        Self {
            db,
            contabilidad,
            ordenes,
            comision_tasa,
        }
    }

    pub async fn manejar(&self, evento: &Value) -> Result<(), String> {
        let tipo = evento
            .get("tipo")
            .and_then(Value::as_str)
            .ok_or_else(|| "evento sin tipo".to_string())?;
        let event_id = evento
            .get("event_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "evento sin event_id".to_string())?
            .to_string();
        if self.ya_procesado(&event_id).await? {
            return Ok(());
        }
        match tipo {
            ORDER_CREATED => self.checkout(evento, &event_id).await?,
            PAYMENT_PROCESADO => self.pago(evento, &event_id).await?,
            COMISION_ACREDITADA => self.comision_acreditada(evento, &event_id).await?,
            DEVOLUCION_SOLICITADA => self.devolucion(evento, &event_id).await?,
            _ => return Ok(()),
        }
        Ok(())
    }

    async fn ya_procesado(&self, event_id: &str) -> Result<bool, String> {
        let uuid = uuid::Uuid::parse_str(event_id).map_err(|_| "event_id invalido".to_string())?;
        let fila: Option<uuid::Uuid> = sqlx::query_scalar(
            "SELECT event_id FROM finance.eventos_procesados WHERE event_id = $1",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| error.to_string())?;
        Ok(fila.is_some())
    }

    async fn checkout(&self, _evento: &Value, event_id: &str) -> Result<(), String> {
        let mut tx = self.db.transaccion().await.map_err(|e| e.to_string())?;
        Metricas::sumar_metrica(&mut tx, "checkouts_completados", 0, 1)
            .await
            .map_err(|e| e.mensaje)?;
        marcar(&mut tx, event_id, ORDER_CREATED).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        logs::info(
            "finance.pedidos",
            crate::campos! { "msg": "Checkout contabilizado para el embudo" },
        );
        Ok(())
    }

    async fn pago(&self, evento: &Value, event_id: &str) -> Result<(), String> {
        let datos = evento
            .get("data")
            .ok_or_else(|| "evento sin data".to_string())?;
        let order_id = datos
            .get("order_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "pago sin order_id".to_string())?;
        let monto = datos
            .get("monto_cents")
            .and_then(Value::as_i64)
            .ok_or_else(|| "pago sin monto_cents".to_string())?;
        let mut tx = self.db.transaccion().await.map_err(|e| e.to_string())?;
        self.contabilidad
            .registrar_en_transaccion(
                &mut tx,
                &ParametrosAsiento {
                    concepto: format!("Pago de la orden {order_id}"),
                    tipo: "INGRESO".to_string(),
                    referencia_tipo: Some("order".to_string()),
                    referencia_id: Some(order_id.to_string()),
                    fecha: None,
                    moneda: None,
                    creado_por: Some("sistema".to_string()),
                    detalles: vec![
                        DetalleAsiento {
                            cuenta_codigo: "1.1.1".to_string(),
                            debe_cents: monto,
                            haber_cents: 0,
                            concepto: Some("Caja".to_string()),
                            orden: None,
                        },
                        DetalleAsiento {
                            cuenta_codigo: "2.1.2".to_string(),
                            debe_cents: 0,
                            haber_cents: monto,
                            concepto: Some("Fondos por liquidar".to_string()),
                            orden: None,
                        },
                    ],
                },
            )
            .await
            .map_err(|e| e.mensaje)?;
        Metricas::sumar_metrica(&mut tx, "pedidos", 0, 1)
            .await
            .map_err(|e| e.mensaje)?;
        Metricas::sumar_metrica(&mut tx, "gmv", monto, 0)
            .await
            .map_err(|e| e.mensaje)?;
        marcar(&mut tx, event_id, PAYMENT_PROCESADO).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        logs::info(
            "finance.pedidos",
            crate::campos! { "msg": "Asiento de pago registrado", "order_id": order_id, "monto_cents": monto },
        );
        Ok(())
    }

    async fn comision_acreditada(&self, evento: &Value, event_id: &str) -> Result<(), String> {
        let datos = evento
            .get("data")
            .ok_or_else(|| "evento sin data".to_string())?;
        let order_id = datos
            .get("order_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "comision sin order_id".to_string())?;
        let monto = datos
            .get("monto_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let monto_vendedor = datos
            .get("monto_vendedor_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let monto_empresario = datos
            .get("monto_empresario_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let vendedor_id = datos
            .get("vendedor_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "comision sin vendedor_id".to_string())?;
        let venta = monto + monto_vendedor + monto_empresario;
        let mut detalles = vec![
            DetalleAsiento {
                cuenta_codigo: "2.1.2".to_string(),
                debe_cents: venta,
                haber_cents: 0,
                concepto: Some("Fondos por liquidar".to_string()),
                orden: None,
            },
            DetalleAsiento {
                cuenta_codigo: "4.1".to_string(),
                debe_cents: 0,
                haber_cents: monto,
                concepto: Some("Comision de la plataforma".to_string()),
                orden: None,
            },
        ];
        if monto_vendedor > 0 {
            detalles.push(DetalleAsiento {
                cuenta_codigo: "2.1.1".to_string(),
                debe_cents: 0,
                haber_cents: monto_vendedor,
                concepto: Some("Liquidacion al vendedor".to_string()),
                orden: None,
            });
        }
        if monto_empresario > 0 {
            detalles.push(DetalleAsiento {
                cuenta_codigo: "2.1.6".to_string(),
                debe_cents: 0,
                haber_cents: monto_empresario,
                concepto: Some("Liquidacion al empresario".to_string()),
                orden: None,
            });
        }
        let mut tx = self.db.transaccion().await.map_err(|e| e.to_string())?;
        self.contabilidad
            .registrar_en_transaccion(
                &mut tx,
                &ParametrosAsiento {
                    concepto: format!(
                        "Realizacion de venta {order_id}: comision (RN-04), vendedor y empresario"
                    ),
                    tipo: "AJUSTE".to_string(),
                    referencia_tipo: Some("order".to_string()),
                    referencia_id: Some(order_id.to_string()),
                    fecha: None,
                    moneda: None,
                    creado_por: Some("sistema".to_string()),
                    detalles,
                },
            )
            .await
            .map_err(|e| e.mensaje)?;
        Metricas::sumar_metrica(&mut tx, "ingresos_comisiones", monto, 0)
            .await
            .map_err(|e| e.mensaje)?;
        Metricas::sumar_metrica(&mut tx, "entregados", 0, 1)
            .await
            .map_err(|e| e.mensaje)?;
        Metricas::incrementar_vendedor(
            &mut tx,
            vendedor_id,
            DeltasVendedor {
                ventas: 1,
                monto_cents: venta,
                comision_cents: monto,
                devoluciones: 0,
            },
        )
        .await
        .map_err(|e| e.mensaje)?;
        marcar(&mut tx, event_id, COMISION_ACREDITADA).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        logs::info(
            "finance.pedidos",
            crate::campos! { "msg": "Asiento de comision registrado", "order_id": order_id, "comision_cents": monto },
        );
        Ok(())
    }

    async fn devolucion(&self, evento: &Value, event_id: &str) -> Result<(), String> {
        let datos = evento
            .get("data")
            .ok_or_else(|| "evento sin data".to_string())?;
        let order_id = datos
            .get("order_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "devolucion sin order_id".to_string())?;
        let orden = self.ordenes.consultar(order_id).await;

        let mut tx = self.db.transaccion().await.map_err(|e| e.to_string())?;
        let previo: Option<uuid::Uuid> = sqlx::query_scalar(
            "SELECT id FROM finance.asientos
             WHERE referencia_tipo = 'order' AND referencia_id = $1
               AND concepto LIKE 'Realizacion de venta%' AND estado = 'REGISTRADO'
             ORDER BY creado_en DESC LIMIT 1",
        )
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        let detalles: Vec<DetalleAsiento> = if let Some(previo) = previo {
            let filas = sqlx::query(
                "SELECT cuenta_codigo, debe_cents, haber_cents
                 FROM finance.asiento_detalles WHERE asiento_id = $1",
            )
            .bind(previo)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            use sqlx::Row;
            filas
                .iter()
                .map(|fila| DetalleAsiento {
                    cuenta_codigo: fila.try_get("cuenta_codigo").unwrap_or_default(),
                    debe_cents: fila.try_get::<i32, _>("haber_cents").unwrap_or(0) as i64,
                    haber_cents: fila.try_get::<i32, _>("debe_cents").unwrap_or(0) as i64,
                    concepto: Some("Reverso por devolucion (RN-06)".to_string()),
                    orden: None,
                })
                .filter(|detalle| detalle.debe_cents > 0 || detalle.haber_cents > 0)
                .collect()
        } else {
            let total = orden
                .map(|orden| orden.total_cents)
                .ok_or_else(|| "Orden no disponible para el asiento de devolucion.".to_string())?;
            let comision =
                money::comision(total, self.comision_tasa).map_err(|error| error.mensaje)?;
            vec![
                DetalleAsiento {
                    cuenta_codigo: "4.1".to_string(),
                    debe_cents: comision,
                    haber_cents: 0,
                    concepto: Some("Comision revertida".to_string()),
                    orden: None,
                },
                DetalleAsiento {
                    cuenta_codigo: "2.1.1".to_string(),
                    debe_cents: 0,
                    haber_cents: comision,
                    concepto: Some("Compensacion en liquidacion".to_string()),
                    orden: None,
                },
            ]
        };

        self.contabilidad
            .registrar_en_transaccion(
                &mut tx,
                &ParametrosAsiento {
                    concepto: format!(
                        "Devolucion de la orden {order_id} (RN-06): reverso del asiento de realizacion"
                    ),
                    tipo: "AJUSTE".to_string(),
                    referencia_tipo: Some("order".to_string()),
                    referencia_id: Some(order_id.to_string()),
                    fecha: None,
                    moneda: None,
                    creado_por: Some("sistema".to_string()),
                    detalles,
                },
            )
            .await
            .map_err(|e| e.mensaje)?;
        Metricas::sumar_metrica(&mut tx, "devoluciones", 0, 1)
            .await
            .map_err(|e| e.mensaje)?;
        marcar(&mut tx, event_id, DEVOLUCION_SOLICITADA).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        logs::info(
            "finance.pedidos",
            crate::campos! { "msg": "Asiento de devolucion registrado", "order_id": order_id },
        );
        Ok(())
    }
}

async fn marcar(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: &str,
    tipo: &str,
) -> Result<(), String> {
    let uuid = uuid::Uuid::parse_str(event_id).map_err(|_| "event_id invalido".to_string())?;
    sqlx::query(
        "INSERT INTO finance.eventos_procesados (event_id, tipo, procesado_en)
         VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(uuid)
    .bind(tipo)
    .execute(&mut **tx)
    .await
    .map_err(|error| error_db(error).mensaje)?;
    Ok(())
}

/// Handler compatible con `Rabbit::consumir`.
pub fn handler_consumidor(consumidor: Arc<Consumidor>) -> crate::rabbit::Handler {
    Arc::new(
        move |evento: Value| -> futures_util::future::BoxFuture<'static, Result<(), String>> {
            let consumidor = Arc::clone(&consumidor);
            Box::pin(async move { consumidor.manejar(&evento).await })
        },
    )
}

/// Errores de dominio del consumidor se traducen a mensajes del bus.
pub fn error_consumidor(error: ApiError) -> String {
    error.mensaje
}
