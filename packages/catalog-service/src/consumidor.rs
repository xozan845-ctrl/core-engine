//! Consumidor de eventos de pedidos (Tabla 22).
//!
//! Réplica de `pedidos.consumer.ts`: el catálogo descuenta stock de forma
//! atómica al crear la orden (RN-03) y revierte stock en devoluciones (RN-06).
//! La idempotencia usa `event_id` en `catalog.eventos_procesados`; stock,
//! eventos de outbox y la marca de idempotencia comparten una transacción.

use crate::db::Db;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::outbox::Outbox;
use crate::productos::Productos;
use futures_util::future::BoxFuture;
use serde_json::{json, Map, Value};
use std::sync::Arc;

const EVENTO_ORDER_CREATED: &str = "order.created";
const EVENTO_DEVOLUCION_SOLICITADA: &str = "devolucion.solicitada";
const EVENTO_STOCK_RESERVADO: &str = "stock.reservado";
const EVENTO_STOCK_FALLIDO: &str = "stock.fallido";
const EVENTO_STOCK_REINTEGRADO: &str = "stock.reintegrado";
const EVENTO_STOCK_UPDATED: &str = "stock.updated";

pub struct PedidosConsumer {
    db: Arc<Db>,
    outbox: Arc<Outbox>,
    productos: Arc<Productos>,
}

impl PedidosConsumer {
    pub fn nuevo(db: Arc<Db>, outbox: Arc<Outbox>, productos: Arc<Productos>) -> Self {
        Self {
            db,
            outbox,
            productos,
        }
    }

    pub async fn manejar(&self, evento: Value) -> Result<(), String> {
        let tipo = evento
            .get("tipo")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if tipo != EVENTO_ORDER_CREATED && tipo != EVENTO_DEVOLUCION_SOLICITADA {
            return Ok(());
        }
        let event_id = evento
            .get("event_id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| "evento sin event_id".to_string())?;
        let contexto = ContextoCorrelacion {
            request_id: format!("catalog-event-{event_id}"),
            servicio: "catalog-consumer".to_string(),
            usuario_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
            rol: Some("super_admin".to_string()),
            tenant_id: None,
        };
        let consumer = self;
        ejecutar_con_contexto(contexto, async move {
            match tipo.as_str() {
                EVENTO_ORDER_CREATED => consumer.orden_creada(&evento, &event_id).await,
                EVENTO_DEVOLUCION_SOLICITADA => consumer.devolucion(&evento, &event_id).await,
                _ => Ok(()),
            }
        })
        .await
    }

    async fn orden_creada(&self, evento: &Value, event_id: &str) -> Result<(), String> {
        let data = evento.get("data").cloned().unwrap_or(Value::Null);
        let order_id = texto(&data, "order_id");
        let tipo_orden = data.get("tipo_orden").and_then(Value::as_str).unwrap_or("");
        let tipo_evento = EVENTO_ORDER_CREATED;

        // Una cotización no compromete inventario: solo se marca procesada.
        if tipo_orden == "cotizacion" {
            let mut tx = self
                .db
                .transaccion()
                .await
                .map_err(|error| error.to_string())?;
            let procesada = if self
                .productos
                .esta_procesado_en_transaccion(&mut tx, event_id)
                .await
                .map_err(mensaje)?
            {
                false
            } else {
                self.productos
                    .registrar_procesado_en_transaccion(&mut tx, event_id, tipo_evento)
                    .await
                    .map_err(mensaje)?;
                true
            };
            tx.commit().await.map_err(|error| error.to_string())?;
            if procesada {
                crate::logs::info(
                    "catalog.pedidos",
                    crate::campos! {
                        "msg": "Cotizacion sin reserva de stock",
                        "order_id": order_id,
                    },
                );
            }
            return Ok(());
        }

        // La entrega digital no compromete stock físico.
        let lineas = lineas_orden(&data);
        let lineas_valor = Value::Array(lineas.clone());

        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(|error| error.to_string())?;
        if self
            .productos
            .esta_procesado_en_transaccion(&mut tx, event_id)
            .await
            .map_err(mensaje)?
        {
            tx.commit().await.map_err(|error| error.to_string())?;
            return Ok(());
        }

        // Stock + eventos + marca de idempotencia comparten UNA transacción:
        // si falla el outbox, el descuento también revierte y el reintento es
        // seguro.
        let reserva = self
            .productos
            .reservar_stock_en_transaccion(&mut tx, &order_id, &lineas_valor)
            .await
            .map_err(mensaje)?;
        let ok = reserva.get("ok").and_then(Value::as_bool).unwrap_or(false);
        if ok {
            let reservados = reserva
                .get("reservados")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_else(|| lineas.clone());
            let items = reservados
                .iter()
                .map(|linea| {
                    json!({
                        "sku": texto(linea, "sku"),
                        "cantidad": linea.get("cantidad").cloned().unwrap_or(json!(0)),
                    })
                })
                .collect::<Vec<_>>();
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    EVENTO_STOCK_RESERVADO,
                    json!({ "order_id": order_id, "items": items }),
                )
                .await
                .map_err(mensaje)?;
            let stock_restante = reserva
                .get("stock_restante")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    EVENTO_STOCK_UPDATED,
                    json!({
                        "order_id": order_id,
                        "tipo": "reservado",
                        "items": stock_restante,
                    }),
                )
                .await
                .map_err(mensaje)?;
        } else {
            let fallidos = reserva
                .get("fallidos")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    EVENTO_STOCK_FALLIDO,
                    json!({ "order_id": order_id, "items": fallidos }),
                )
                .await
                .map_err(mensaje)?;
        }
        self.productos
            .registrar_procesado_en_transaccion(&mut tx, event_id, tipo_evento)
            .await
            .map_err(mensaje)?;
        tx.commit().await.map_err(|error| error.to_string())?;

        let fallidos = reserva
            .get("fallidos")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        crate::logs::info(
            "catalog.pedidos",
            crate::campos! {
                "msg": if ok { "Stock reservado atomico" } else { "Stock insuficiente" },
                "order_id": order_id,
                "fallidos": fallidos,
            },
        );
        Ok(())
    }

    async fn devolucion(&self, evento: &Value, event_id: &str) -> Result<(), String> {
        let data = evento.get("data").cloned().unwrap_or(Value::Null);
        let order_id = texto(&data, "order_id");
        let lineas = lineas_devolucion(&data);
        let lineas_valor = Value::Array(lineas.clone());
        let claves = claves_stock(&lineas);
        let tipo_evento = EVENTO_DEVOLUCION_SOLICITADA;

        let mut tx = self
            .db
            .transaccion()
            .await
            .map_err(|error| error.to_string())?;
        if self
            .productos
            .esta_procesado_en_transaccion(&mut tx, event_id)
            .await
            .map_err(mensaje)?
        {
            tx.commit().await.map_err(|error| error.to_string())?;
            return Ok(());
        }

        self.productos
            .reintegrar_stock_en_transaccion(&mut tx, &order_id, &lineas_valor)
            .await
            .map_err(mensaje)?;
        self.productos
            .devolver_series_en_transaccion(&mut tx, &order_id, &lineas_valor)
            .await
            .map_err(mensaje)?;
        let items = lineas
            .iter()
            .map(|linea| {
                json!({
                    "sku": texto(linea, "sku"),
                    "cantidad": linea.get("cantidad").cloned().unwrap_or(json!(0)),
                })
            })
            .collect::<Vec<_>>();
        self.outbox
            .insertar_en_transaccion(
                &mut tx,
                EVENTO_STOCK_REINTEGRADO,
                json!({ "order_id": order_id, "items": items }),
            )
            .await
            .map_err(mensaje)?;
        // stock.updated tras la devolución (RN-06 revierte stock; RN-02
        // actualiza ofertas).
        let stock = self
            .productos
            .stock_actual_de_en_transaccion(&mut tx, &claves)
            .await
            .map_err(mensaje)?;
        self.outbox
            .insertar_en_transaccion(
                &mut tx,
                EVENTO_STOCK_UPDATED,
                json!({
                    "order_id": order_id,
                    "tipo": "reintegrado",
                    "items": stock,
                }),
            )
            .await
            .map_err(mensaje)?;
        self.productos
            .registrar_procesado_en_transaccion(&mut tx, event_id, tipo_evento)
            .await
            .map_err(mensaje)?;
        tx.commit().await.map_err(|error| error.to_string())?;

        crate::logs::info(
            "catalog.pedidos",
            crate::campos! {
                "msg": "Stock reintegrado por devolucion",
                "order_id": order_id,
            },
        );
        Ok(())
    }
}

/// Handler compatible con el cliente RabbitMQ.
pub fn handler_consumidor(consumidor: Arc<PedidosConsumer>) -> crate::rabbit::Handler {
    Arc::new(
        move |evento: Value| -> BoxFuture<'static, Result<(), String>> {
            let consumidor = Arc::clone(&consumidor);
            Box::pin(async move { consumidor.manejar(evento).await })
        },
    )
}

/// Cola del consumidor de pedidos del catálogo (Tabla 22).
pub fn cola() -> crate::rabbit::ColaConfig {
    crate::rabbit::ColaConfig {
        nombre: "catalog.pedidos".to_string(),
        routing_keys: vec![
            EVENTO_ORDER_CREATED.to_string(),
            EVENTO_DEVOLUCION_SOLICITADA.to_string(),
        ],
    }
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn mensaje(error: crate::errors::ApiError) -> String {
    error.mensaje
}

/// Líneas de reserva: excluye la entrega digital y conserva `item_id` cuando
/// el pedido lo informa (expansión de composición BOM).
fn lineas_orden(data: &Value) -> Vec<Value> {
    data.get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get("fulfillment").and_then(Value::as_str) != Some("digital"))
                .map(|item| {
                    let mut linea = Map::new();
                    linea.insert(
                        "empresa_id".into(),
                        Value::String(texto(item, "empresa_id")),
                    );
                    linea.insert("sku".into(), Value::String(texto(item, "sku")));
                    linea.insert(
                        "cantidad".into(),
                        item.get("cantidad").cloned().unwrap_or(json!(0)),
                    );
                    if let Some(item_id) = item
                        .get("item_id")
                        .and_then(Value::as_str)
                        .filter(|valor| !valor.is_empty())
                    {
                        linea.insert("item_id".into(), Value::String(item_id.to_string()));
                    }
                    Value::Object(linea)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Líneas de devolución: solo `(empresa, sku, cantidad)`.
fn lineas_devolucion(data: &Value) -> Vec<Value> {
    data.get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    json!({
                        "empresa_id": texto(item, "empresa_id"),
                        "sku": texto(item, "sku"),
                        "cantidad": item.get("cantidad").cloned().unwrap_or(json!(0)),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Claves únicas `(empresa, sku)` para releer el stock tras la devolución.
fn claves_stock(lineas: &[Value]) -> Value {
    let mut salida = Vec::new();
    let mut vistas = std::collections::HashSet::new();
    for linea in lineas {
        let empresa_id = texto(linea, "empresa_id");
        let sku = texto(linea, "sku").to_lowercase();
        if vistas.insert(format!("{empresa_id}:{sku}")) {
            salida.push(json!({ "empresa_id": empresa_id, "sku": sku }));
        }
    }
    Value::Array(salida)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_declarar_la_cola_y_el_routing_del_contrato() {
        let cola = cola();
        assert_eq!(cola.nombre, "catalog.pedidos");
        assert_eq!(
            cola.routing_keys,
            vec![
                "order.created".to_string(),
                "devolucion.solicitada".to_string()
            ]
        );
    }

    #[test]
    fn debe_excluir_la_entrega_digital_y_conservar_item_id() {
        let lineas = lineas_orden(&json!({
            "items": [
                {"empresa_id": "e-1", "sku": "SKU-01", "cantidad": 2, "item_id": "i-1"},
                {"empresa_id": "e-1", "sku": "DIGITAL", "cantidad": 1, "fulfillment": "digital"},
                {"empresa_id": "e-1", "sku": "SKU-02", "cantidad": 3, "item_id": ""}
            ]
        }));
        assert_eq!(
            lineas,
            vec![
                json!({"empresa_id": "e-1", "sku": "SKU-01", "cantidad": 2, "item_id": "i-1"}),
                json!({"empresa_id": "e-1", "sku": "SKU-02", "cantidad": 3}),
            ]
        );
    }

    #[test]
    fn debe_construir_lineas_de_devolucion_sin_item_id() {
        let lineas = lineas_devolucion(&json!({
            "items": [
                {"empresa_id": "e-1", "sku": "SKU-01", "cantidad": 1, "item_id": "i-1"}
            ]
        }));
        assert_eq!(
            lineas,
            vec![json!({"empresa_id": "e-1", "sku": "SKU-01", "cantidad": 1})]
        );
    }

    #[test]
    fn debe_deduplicar_las_claves_de_stock_de_la_devolucion() {
        let lineas = vec![
            json!({"empresa_id": "e-1", "sku": "SKU-01", "cantidad": 1}),
            json!({"empresa_id": "e-1", "sku": "sku-01", "cantidad": 2}),
            json!({"empresa_id": "e-2", "sku": "SKU-01", "cantidad": 1}),
        ];
        assert_eq!(
            claves_stock(&lineas),
            json!([
                {"empresa_id": "e-1", "sku": "sku-01"},
                {"empresa_id": "e-2", "sku": "sku-01"}
            ])
        );
    }
}
