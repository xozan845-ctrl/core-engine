//! Consumidor de eventos de pedidos (Tabla 22).
//!
//! - `payment.procesado` registra el pago (entidad Pagos, simulado en MVP).
//! - `order.completado` acredita la comisión por línea (RN-04) y emite
//!   `comision.acreditada` para la liquidación quincenal.
//! - `devolucion.solicitada` compensa la comisión (RN-06).
//!
//! Idempotencia por `event_id` (y por orden/línea en `acreditar_linea`).

use crate::campos;
use crate::clients::Ordenes;
use crate::comisiones::{Comisiones, DatosLinea, DatosPago};
use crate::db::{error_db, parsear_uuid, Db};
use crate::errors::ApiError;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::outbox::{Outbox, COMISION_ACREDITADA};
use crate::rabbit::Handler;
use futures_util::future::BoxFuture;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
use std::sync::Arc;

pub const EVENTO_PAYMENT_PROCESADO: &str = "payment.procesado";
pub const EVENTO_ORDER_COMPLETADO: &str = "order.completado";
pub const EVENTO_DEVOLUCION_SOLICITADA: &str = "devolucion.solicitada";
pub const EVENTO_DEVOLUCION_RECIBIDA: &str = "devolucion.recibida";
pub const USUARIO_SISTEMA: &str = "00000000-0000-0000-0000-000000000000";

pub struct PedidosConsumer {
    db: Arc<Db>,
    comisiones: Arc<Comisiones>,
    outbox: Outbox,
    ordenes: Arc<dyn Ordenes>,
}

impl PedidosConsumer {
    pub fn nuevo(
        db: Arc<Db>,
        comisiones: Arc<Comisiones>,
        outbox: Outbox,
        ordenes: Arc<dyn Ordenes>,
    ) -> Self {
        Self {
            db,
            comisiones,
            outbox,
            ordenes,
        }
    }

    pub async fn manejar_liquidacion(&self, evento: &Value) -> Result<(), ApiError> {
        if evento.get("tipo").and_then(Value::as_str) != Some(COMISION_ACREDITADA) {
            return Ok(());
        }
        crate::logs::info(
            "commissions.pedidos",
            campos! {
                "msg": "Comision disponible para liquidacion",
                "event_id": evento.get("event_id").and_then(Value::as_str).unwrap_or_default(),
            },
        );
        Ok(())
    }

    pub async fn manejar(&self, evento: Value) -> Result<(), ApiError> {
        let Some(tipo) = evento
            .get("tipo")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return Ok(());
        };
        if !matches!(
            tipo.as_str(),
            EVENTO_PAYMENT_PROCESADO
                | EVENTO_ORDER_COMPLETADO
                | EVENTO_DEVOLUCION_SOLICITADA
                | EVENTO_DEVOLUCION_RECIBIDA
        ) {
            return Ok(());
        }
        let event_id = campo(&evento, "event_id")?.to_string();
        let contexto = ContextoCorrelacion {
            request_id: format!("commissions-event-{event_id}"),
            servicio: "commissions-consumer".to_string(),
            usuario_id: Some(USUARIO_SISTEMA.to_string()),
            rol: Some("super_admin".to_string()),
            tenant_id: None,
            negocio_id: None,
        };
        ejecutar_con_contexto(contexto, async {
            match tipo.as_str() {
                EVENTO_PAYMENT_PROCESADO => self.pago(&evento, &event_id).await,
                EVENTO_ORDER_COMPLETADO => self.orden_completada(&evento, &event_id).await,
                EVENTO_DEVOLUCION_SOLICITADA => self.devolucion(&evento, &event_id).await,
                EVENTO_DEVOLUCION_RECIBIDA => self.devolucion_recibida(&evento, &event_id).await,
                _ => Ok(()),
            }
        })
        .await
    }

    async fn pago(&self, evento: &Value, event_id: &str) -> Result<(), ApiError> {
        if self.ya_procesado(event_id).await? {
            return Ok(());
        }
        let data = datos(evento)?;
        let datos_pago = DatosPago {
            order_id: campo(data, "order_id")?.to_string(),
            monto_cents: entero(data, "monto_cents")?,
            metodo: texto_opcional(data, "metodo").unwrap_or_default(),
            proveedor: texto_opcional(data, "proveedor").unwrap_or_default(),
            moneda: texto_opcional(data, "moneda").unwrap_or_default(),
            comision_pasarela_cents: entero_opcional(data, "comision_pasarela_cents"),
            impuesto_pasarela_cents: entero_opcional(data, "impuesto_pasarela_cents"),
            retencion_cents: entero_opcional(data, "retencion_cents"),
            monto_liquidado_cents: entero_opcional(data, "monto_liquidado_cents"),
            moneda_liquidacion: texto_opcional(data, "moneda_liquidacion").unwrap_or_default(),
        };
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        self.comisiones.registrar_pago(&mut tx, &datos_pago).await?;
        self.marcar_procesado(&mut tx, event_id, EVENTO_PAYMENT_PROCESADO)
            .await?;
        tx.commit().await.map_err(error_db)?;
        Ok(())
    }

    async fn orden_completada(&self, evento: &Value, event_id: &str) -> Result<(), ApiError> {
        if self.ya_procesado(event_id).await? {
            return Ok(());
        }
        let data = datos(evento)?;
        let order_id = campo(data, "order_id")?.to_string();
        let orden = self.ordenes.obtener(&order_id).await.ok_or_else(|| {
            ApiError::interno_mensaje("Orden no disponible para acreditar comisión.")
        })?;
        if orden.items.is_empty() {
            return Err(ApiError::interno_mensaje(
                "La orden no tiene líneas asignadas.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        if self.ya_procesado_tx(&mut tx, event_id).await? {
            tx.commit().await.map_err(error_db)?;
            return Ok(());
        }
        let mut acreditadas = 0usize;
        let mut ventas_directas = 0usize;
        for item in &orden.items {
            let venta_cents = item
                .precio_unitario_cents
                .checked_mul(item.cantidad)
                .and_then(|bruto| bruto.checked_sub(item.descuento_cents))
                .ok_or_else(|| ApiError::interno_mensaje("La línea desborda el monto."))?
                .max(0);
            let Some(empresa_id) = item.empresa_id.clone() else {
                return Err(ApiError::interno_mensaje(format!(
                    "La línea {} no tiene empresa para contabilizar la venta.",
                    item.oferta_id
                )));
            };
            if !item.comision || item.vendedor_id.is_none() {
                self.outbox
                    .insertar_en_transaccion(
                        &mut tx,
                        "venta.directa.acreditada",
                        serde_json::json!({
                            "order_id": order_id,
                            "linea_clave": item.oferta_id,
                            "empresa_id": empresa_id,
                            "negocio_id": item.negocio_id,
                            "base_cents": venta_cents,
                            "impuesto_cents": item.impuesto_cents,
                            "cantidad": item.cantidad,
                        }),
                    )
                    .await?;
                ventas_directas += 1;
                continue;
            }
            let (Some(vendedor_id), Some(tienda_id), Some(participacion)) = (
                item.vendedor_id.clone(),
                item.tienda_id.clone(),
                item.participacion_porcentaje,
            ) else {
                return Err(ApiError::interno_mensaje(format!(
                    "La línea comisionable {} no tiene vendedor/tienda/participación.",
                    item.oferta_id
                )));
            };
            let comision = self
                .comisiones
                .acreditar_linea(
                    &mut tx,
                    &DatosLinea {
                        order_id: order_id.clone(),
                        linea_clave: item.oferta_id.clone(),
                        empresa_id: empresa_id.clone(),
                        negocio_id: item.negocio_id.clone(),
                        tienda_id: tienda_id.clone(),
                        vendedor_id: vendedor_id.clone(),
                        venta_cents,
                        participacion,
                        cantidad: item.cantidad,
                    },
                )
                .await?;
            let Some(comision) = comision else {
                continue;
            };
            let payload = serde_json::json!({
                "order_id": order_id,
                "linea_clave": item.oferta_id,
                "base_cents": venta_cents,
                "impuesto_cents": item.impuesto_cents,
                "cantidad": item.cantidad,
                "monto_cents": comision.comision_cents,
                "monto_vendedor_cents": comision.monto_vendedor_cents,
                "monto_empresario_cents": comision.monto_empresario_cents.unwrap_or(0),
                "vendedor_id": vendedor_id,
                "empresa_id": empresa_id,
                "negocio_id": comision.negocio_id,
                "tienda_id": tienda_id,
                "participacion_porcentaje": participacion,
            });
            self.outbox
                .insertar_en_transaccion(&mut tx, COMISION_ACREDITADA, payload)
                .await?;
            acreditadas += 1;
        }
        self.marcar_procesado(&mut tx, event_id, EVENTO_ORDER_COMPLETADO)
            .await?;
        tx.commit().await.map_err(error_db)?;
        crate::logs::info(
            "commissions.pedidos",
            campos! {
                "msg": "Ventas del pedido procesadas",
                "order_id": order_id,
                "lineas_comisionadas": acreditadas,
                "lineas_directas": ventas_directas,
            },
        );
        Ok(())
    }

    async fn devolucion_recibida(&self, evento: &Value, event_id: &str) -> Result<(), ApiError> {
        if self.ya_procesado(event_id).await? {
            return Ok(());
        }
        let data = datos(evento)?;
        let order_id = campo(data, "order_id")?;
        let items = data
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        if self.ya_procesado_tx(&mut tx, event_id).await? {
            tx.commit().await.map_err(error_db)?;
            return Ok(());
        }
        for item in &items {
            let linea = item
                .get("linea_clave")
                .and_then(Value::as_str)
                .unwrap_or("");
            let cantidad = item.get("cantidad").and_then(Value::as_i64).unwrap_or(0);
            self.comisiones
                .compensar_linea(&mut tx, order_id, linea, cantidad, "devolucion parcial")
                .await?;
        }
        self.marcar_procesado(&mut tx, event_id, EVENTO_DEVOLUCION_RECIBIDA)
            .await?;
        tx.commit().await.map_err(error_db)?;
        Ok(())
    }

    async fn devolucion(&self, evento: &Value, event_id: &str) -> Result<(), ApiError> {
        if self.ya_procesado(event_id).await? {
            return Ok(());
        }
        let data = datos(evento)?;
        let order_id = campo(data, "order_id")?;
        let motivo = campo(data, "motivo")?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        self.comisiones.compensar(&mut tx, order_id, motivo).await?;
        self.marcar_procesado(&mut tx, event_id, EVENTO_DEVOLUCION_SOLICITADA)
            .await?;
        tx.commit().await.map_err(error_db)?;
        Ok(())
    }

    async fn ya_procesado(&self, event_id: &str) -> Result<bool, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query("SELECT 1 FROM commissions.eventos_procesados WHERE event_id = $1")
            .bind(parsear_uuid(event_id)?)
            .fetch_optional(&mut *tx)
            .await
            .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        Ok(fila.is_some())
    }

    async fn ya_procesado_tx(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
    ) -> Result<bool, ApiError> {
        let fila = sqlx::query("SELECT 1 FROM commissions.eventos_procesados WHERE event_id = $1")
            .bind(parsear_uuid(event_id)?)
            .fetch_optional(&mut **tx)
            .await
            .map_err(error_db)?;
        Ok(fila.is_some())
    }

    async fn marcar_procesado(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        event_id: &str,
        tipo: &str,
    ) -> Result<(), ApiError> {
        sqlx::query(
            "INSERT INTO commissions.eventos_procesados (event_id, tipo, procesado_en)
             VALUES ($1, $2, NOW()) ON CONFLICT (event_id) DO NOTHING",
        )
        .bind(parsear_uuid(event_id)?)
        .bind(tipo)
        .execute(&mut **tx)
        .await
        .map_err(error_db)?;
        Ok(())
    }
}

pub fn handler_consumidor(consumidor: Arc<PedidosConsumer>) -> Handler {
    Arc::new(move |evento| {
        let consumidor = Arc::clone(&consumidor);
        Box::pin(async move {
            consumidor
                .manejar(evento)
                .await
                .map_err(|error| error.to_string())
        }) as BoxFuture<'static, Result<(), String>>
    })
}

pub fn handler_liquidacion(consumidor: Arc<PedidosConsumer>) -> Handler {
    Arc::new(move |evento| {
        let consumidor = Arc::clone(&consumidor);
        Box::pin(async move {
            consumidor
                .manejar_liquidacion(&evento)
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

fn campo<'a>(objeto: &'a Value, clave: &str) -> Result<&'a str, ApiError> {
    objeto
        .get(clave)
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::interno_mensaje(format!("Campo invalido: {clave}.")))
}

fn entero(objeto: &Value, clave: &str) -> Result<i64, ApiError> {
    objeto
        .get(clave)
        .and_then(Value::as_i64)
        .ok_or_else(|| ApiError::interno_mensaje(format!("Campo invalido: {clave}.")))
}

fn texto_opcional(objeto: &Value, clave: &str) -> Option<String> {
    objeto
        .get(clave)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(ToString::to_string)
}

fn entero_opcional(objeto: &Value, clave: &str) -> i64 {
    objeto.get(clave).and_then(Value::as_i64).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_extraer_los_campos_del_evento() {
        let evento = json!({
            "event_id": "00000000-0000-0000-0000-000000000001",
            "tipo": "payment.procesado",
            "data": { "order_id": "o-1", "monto_cents": 2500 }
        });
        let data = datos(&evento).expect("data");
        assert_eq!(campo(data, "order_id").expect("order"), "o-1");
        assert_eq!(entero(data, "monto_cents").expect("monto"), 2500);
    }

    #[test]
    fn debe_fallar_ante_campos_invalidos() {
        let evento = json!({ "event_id": "x", "tipo": "payment.procesado" });
        assert_eq!(
            datos(&evento).expect_err("sin data").codigo,
            "ERROR_INTERNO"
        );
        let data = json!({ "order_id": 5 });
        assert!(campo(&data, "order_id").is_err());
        assert!(entero(&data, "monto_cents").is_err());
    }
}
