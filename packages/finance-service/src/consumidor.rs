//! Consumidor de eventos (tabla 22) que alimenta la contabilidad y el tablero:
//! - `order.created`       -> embudo: checkout completado
//! - `payment.procesado`   -> asiento Caja / Fondos por liquidar + métrica GMV
//! - `comision.acreditada` -> realización del ingreso (RN-04)
//! - `venta.directa.acreditada` -> la venta directa queda como acreedor del
//!   empresario, nunca como ingreso de la plataforma
//! - `devolucion.solicitada` -> reverso total del asiento de realización (RN-06)
//! - `devolucion.recibida` -> reverso proporcional de la liquidación por RMA
//! - `devolucion.reembolsada` -> salida de caja solo cuando la pasarela
//!   confirma el reembolso
//!
//! Idempotencia por `event_id` y persistencia atómica con el Outbox (ADR-03).

use crate::contabilidad::{Contabilidad, DetalleAsiento, ParametrosAsiento};
use crate::db::Db;
use crate::errors::{error_db, ApiError};
use crate::facturacion::Ordenes;
use crate::logs;
use crate::metricas::{DeltasVendedor, Metricas};
use serde_json::Value;
use std::sync::Arc;

pub const ORDER_CREATED: &str = "order.created";
pub const PAYMENT_PROCESADO: &str = "payment.procesado";
pub const COMISION_ACREDITADA: &str = "comision.acreditada";
pub const VENTA_DIRECTA_ACREDITADA: &str = "venta.directa.acreditada";
pub const DEVOLUCION_SOLICITADA: &str = "devolucion.solicitada";
pub const DEVOLUCION_RECIBIDA: &str = "devolucion.recibida";
pub const DEVOLUCION_REEMBOLSADA: &str = "devolucion.reembolsada";

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
            VENTA_DIRECTA_ACREDITADA => self.venta_directa(evento, &event_id).await?,
            DEVOLUCION_SOLICITADA => self.devolucion(evento, &event_id).await?,
            DEVOLUCION_RECIBIDA => self.devolucion_recibida(evento, &event_id).await?,
            DEVOLUCION_REEMBOLSADA => self.devolucion_reembolsada(evento, &event_id).await?,
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
        let empresa_id = datos.get("empresa_id").and_then(Value::as_str);
        let negocio_id = datos.get("negocio_id").and_then(Value::as_str);
        let linea_clave = datos
            .get("linea_clave")
            .and_then(Value::as_str)
            .unwrap_or(order_id);
        let venta = monto + monto_vendedor + monto_empresario;
        let base_cents = datos
            .get("base_cents")
            .and_then(Value::as_i64)
            .unwrap_or(venta);
        let impuesto_cents = datos
            .get("impuesto_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let cantidad = datos
            .get("cantidad")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            .max(1);
        let mut detalles = vec![
            DetalleAsiento {
                cuenta_codigo: "2.1.2".to_string(),
                debe_cents: base_cents,
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
        let nueva = if let Some(empresa_id) = empresa_id {
            sqlx::query_scalar::<_, i32>(
                "INSERT INTO finance.ventas_lineas
                   (order_id, linea_clave, empresa_id, negocio_id, directa, base_cents,
                    impuesto_cents, comision_cents, vendedor_cents, empresario_cents, cantidad)
                 VALUES ($1::uuid, $2, $3::uuid, $4::uuid, false, $5, $6, $7, $8, $9, $10)
                 ON CONFLICT (order_id, linea_clave) DO NOTHING
                 RETURNING 1",
            )
            .bind(order_id)
            .bind(linea_clave)
            .bind(empresa_id)
            .bind(negocio_id)
            .bind(base_cents as i32)
            .bind(impuesto_cents as i32)
            .bind(monto as i32)
            .bind(monto_vendedor as i32)
            .bind(monto_empresario as i32)
            .bind(cantidad as i32)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .is_some()
        } else {
            true
        };
        if nueva {
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
        }
        marcar(&mut tx, event_id, COMISION_ACREDITADA).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        logs::info(
            "finance.pedidos",
            crate::campos! { "msg": "Asiento de comision registrado", "order_id": order_id, "comision_cents": monto },
        );
        Ok(())
    }

    /// Venta directa: la plataforma cobra por cuenta del empresario. El importe
    /// completo (base + impuesto) queda como acreedor del empresario; no se
    /// reconoce ingreso por comisión porque no existe comisión de plataforma.
    async fn venta_directa(&self, evento: &Value, event_id: &str) -> Result<(), String> {
        let datos = evento
            .get("data")
            .ok_or_else(|| "evento sin data".to_string())?;
        let order_id = datos
            .get("order_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "venta directa sin order_id".to_string())?;
        let linea_clave = datos
            .get("linea_clave")
            .and_then(Value::as_str)
            .unwrap_or(order_id);
        let empresa_id = datos
            .get("empresa_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "venta directa sin empresa_id".to_string())?;
        let negocio_id = datos.get("negocio_id").and_then(Value::as_str);
        let base_cents = datos
            .get("base_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0);
        let impuesto_cents = datos
            .get("impuesto_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0);
        let cantidad = datos
            .get("cantidad")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            .max(1);
        let total = base_cents + impuesto_cents;
        let mut tx = self.db.transaccion().await.map_err(|e| e.to_string())?;
        let nueva = sqlx::query_scalar::<_, i32>(
            "INSERT INTO finance.ventas_lineas
               (order_id, linea_clave, empresa_id, negocio_id, directa, base_cents,
                impuesto_cents, comision_cents, vendedor_cents, empresario_cents, cantidad)
             VALUES ($1::uuid, $2, $3::uuid, $4::uuid, true, $5, $6, 0, 0, $7, $8)
             ON CONFLICT (order_id, linea_clave) DO NOTHING
             RETURNING 1",
        )
        .bind(order_id)
        .bind(linea_clave)
        .bind(empresa_id)
        .bind(negocio_id)
        .bind(base_cents as i32)
        .bind(impuesto_cents as i32)
        .bind(total as i32)
        .bind(cantidad as i32)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?
        .is_some();
        if nueva && total > 0 {
            self.contabilidad
                .registrar_en_transaccion(
                    &mut tx,
                    &ParametrosAsiento {
                        concepto: format!(
                            "Realizacion de venta directa {order_id} (linea {linea_clave}): acreedor del empresario"
                        ),
                        tipo: "AJUSTE".to_string(),
                        referencia_tipo: Some("order".to_string()),
                        referencia_id: Some(order_id.to_string()),
                        fecha: None,
                        moneda: None,
                        creado_por: Some("sistema".to_string()),
                        detalles: vec![
                            DetalleAsiento {
                                cuenta_codigo: "2.1.2".to_string(),
                                debe_cents: total,
                                haber_cents: 0,
                                concepto: Some("Fondos por liquidar".to_string()),
                                orden: None,
                            },
                            DetalleAsiento {
                                cuenta_codigo: "2.1.6".to_string(),
                                debe_cents: 0,
                                haber_cents: total,
                                concepto: Some("Liquidacion al empresario".to_string()),
                                orden: None,
                            },
                        ],
                    },
                )
                .await
                .map_err(|e| e.mensaje)?;
        }
        if nueva {
            Metricas::sumar_metrica(&mut tx, "entregados", 0, 1)
                .await
                .map_err(|e| e.mensaje)?;
        }
        marcar(&mut tx, event_id, VENTA_DIRECTA_ACREDITADA).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        logs::info(
            "finance.pedidos",
            crate::campos! {
                "msg": "Venta directa registrada como acreedor del empresario",
                "order_id": order_id,
                "total_cents": total,
            },
        );
        Ok(())
    }

    /// Reverso total (cancelación de orden pagada o devolución logística).
    /// Usa el subledger por línea para no revertir dos veces lo ya devuelto.
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
        let tiene_subledger = crate::reversos::tiene_subledger(&mut tx, order_id).await?;
        let mut detalles = crate::reversos::reverso_realizacion(&mut tx, order_id, None).await?;
        if detalles.is_empty() && !tiene_subledger {
            detalles =
                crate::reversos::reverso_legado(&mut tx, order_id, orden, self.comision_tasa)
                    .await?;
        }
        if !detalles.is_empty() {
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
        }
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

    /// RMA recibida: revierte proporcionalmente la liquidación de las líneas
    /// devueltas. No toca Caja: el reembolso se asienta cuando la pasarela lo
    /// confirma (`devolucion.reembolsada`).
    async fn devolucion_recibida(&self, evento: &Value, event_id: &str) -> Result<(), String> {
        let datos = evento
            .get("data")
            .ok_or_else(|| "evento sin data".to_string())?;
        let order_id = datos
            .get("order_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "devolucion recibida sin order_id".to_string())?;
        let items = datos
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let cantidades: std::collections::HashMap<String, i64> = items
            .iter()
            .filter_map(|item| {
                let linea = item.get("linea_clave").and_then(Value::as_str)?;
                let cantidad = item.get("cantidad").and_then(Value::as_i64).unwrap_or(0);
                Some((linea.to_string(), cantidad))
            })
            .collect();

        let mut tx = self.db.transaccion().await.map_err(|e| e.to_string())?;
        let tiene_subledger = crate::reversos::tiene_subledger(&mut tx, order_id).await?;
        let mut detalles =
            crate::reversos::reverso_realizacion(&mut tx, order_id, Some(&cantidades)).await?;
        if detalles.is_empty() && !tiene_subledger {
            // Órdenes anteriores al subledger: al menos deja constancia de la
            // salida de la liquidación usando los importes del propio evento.
            detalles = crate::reversos::reverso_desde_evento(&items);
        }
        if !detalles.is_empty() {
            self.contabilidad
                .registrar_en_transaccion(
                    &mut tx,
                    &ParametrosAsiento {
                        concepto: format!(
                            "Devolucion RMA de la orden {order_id}: reverso proporcional de la liquidacion"
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
        }
        marcar(&mut tx, event_id, DEVOLUCION_RECIBIDA).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        logs::info(
            "finance.pedidos",
            crate::campos! { "msg": "Reverso proporcional de RMA registrado", "order_id": order_id },
        );
        Ok(())
    }

    /// La pasarela confirmó el reembolso: recién aquí sale dinero de Caja.
    /// Usa el acumulado del evento para que una reentrega o un evento fuera de
    /// orden no asiente dos veces el mismo monto.
    async fn devolucion_reembolsada(&self, evento: &Value, event_id: &str) -> Result<(), String> {
        let datos = evento
            .get("data")
            .ok_or_else(|| "evento sin data".to_string())?;
        let order_id = datos
            .get("order_id")
            .and_then(Value::as_str)
            .ok_or_else(|| "reembolso sin order_id".to_string())?;
        let devolucion_id = datos
            .get("devolucion_id")
            .and_then(Value::as_str)
            .unwrap_or(order_id);
        let empresa_id = datos.get("empresa_id").and_then(Value::as_str);
        let monto = datos
            .get("monto_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0);
        let acumulado = datos
            .get("reembolsado_cents")
            .and_then(Value::as_i64)
            .map(|valor| valor.max(0));
        let mut tx = self.db.transaccion().await.map_err(|e| e.to_string())?;

        let insertado: Option<i32> = sqlx::query_scalar(
            "INSERT INTO finance.devoluciones_reembolsadas
               (devolucion_id, order_id, empresa_id, reembolsado_cents)
             VALUES ($1::uuid, $2::uuid, $3::uuid, 0)
             ON CONFLICT (devolucion_id) DO NOTHING
             RETURNING 1",
        )
        .bind(devolucion_id)
        .bind(order_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let previo: i64 = if insertado.is_some() {
            0
        } else {
            sqlx::query_scalar(
                "SELECT reembolsado_cents::bigint
                   FROM finance.devoluciones_reembolsadas
                  WHERE devolucion_id = $1::uuid
                  FOR UPDATE",
            )
            .bind(devolucion_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
        };
        let nuevo = match acumulado {
            Some(total) => total.max(previo),
            None => previo + monto,
        };
        let delta = (nuevo - previo).max(0);
        if delta > 0 {
            self.contabilidad
                .registrar_en_transaccion(
                    &mut tx,
                    &ParametrosAsiento {
                        concepto: format!(
                            "Reembolso confirmado de la devolucion {devolucion_id} (orden {order_id})"
                        ),
                        tipo: "EGRESO".to_string(),
                        referencia_tipo: Some("order".to_string()),
                        referencia_id: Some(order_id.to_string()),
                        fecha: None,
                        moneda: None,
                        creado_por: Some("sistema".to_string()),
                        detalles: vec![
                            DetalleAsiento {
                                cuenta_codigo: "2.1.2".to_string(),
                                debe_cents: delta,
                                haber_cents: 0,
                                concepto: Some("Fondos por liquidar".to_string()),
                                orden: None,
                            },
                            DetalleAsiento {
                                cuenta_codigo: "1.1.1".to_string(),
                                debe_cents: 0,
                                haber_cents: delta,
                                concepto: Some("Salida de caja por reembolso".to_string()),
                                orden: None,
                            },
                        ],
                    },
                )
                .await
                .map_err(|e| e.mensaje)?;
        }
        if nuevo > previo {
            sqlx::query(
                "UPDATE finance.devoluciones_reembolsadas
                    SET reembolsado_cents = $2, actualizado_en = NOW()
                  WHERE devolucion_id = $1::uuid",
            )
            .bind(devolucion_id)
            .bind(nuevo as i32)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
        }
        marcar(&mut tx, event_id, DEVOLUCION_REEMBOLSADA).await?;
        tx.commit().await.map_err(|e| e.to_string())?;
        logs::info(
            "finance.pedidos",
            crate::campos! {
                "msg": "Reembolso confirmado contabilizado",
                "order_id": order_id,
                "monto_cents": delta,
            },
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
