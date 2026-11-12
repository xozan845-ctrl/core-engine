//! Cliente RabbitMQ (AD-02/AD-04): exchanges topic, DLQ, reintentos y
//! republicación automática. Réplica de `rabbit.service.ts`.

use crate::campos;
use crate::logs;
use futures_util::future::BoxFuture;
use futures_util::StreamExt;
use lapin::message::Delivery;
use lapin::options::{
    BasicAckOptions, BasicConsumeOptions, BasicGetOptions, BasicNackOptions, BasicPublishOptions,
    BasicQosOptions, ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions,
};
use lapin::types::{AMQPValue, FieldTable, ShortString};
use lapin::{BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub const EXCHANGE_EVENTS: &str = "core-engine.events";
pub const EXCHANGE_DLQ: &str = "core-engine.dlq";
pub const REINTENTOS_MAXIMOS: u32 = 3;
pub const BACKOFF_BASE_MS: u64 = 500;
const RECONEXION_BASE_MS: u64 = 1_000;
const RECONEXION_MAX_MS: u64 = 30_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColaConfig {
    pub nombre: String,
    pub routing_keys: Vec<String>,
}

pub type Handler = Arc<dyn Fn(Value) -> BoxFuture<'static, Result<(), String>> + Send + Sync>;

struct Estado {
    canal: Option<Channel>,
    conectado: bool,
}

pub struct Rabbit {
    url: String,
    consumidor: String,
    estado: Arc<Mutex<Estado>>,
    consumidores: Mutex<HashMap<String, Handler>>,
    colas: Mutex<Vec<ColaConfig>>,
}

impl Rabbit {
    pub fn nueva(url: &str, consumidor: &str) -> Self {
        let url = if url.contains("heartbeat=") {
            url.to_string()
        } else {
            format!(
                "{url}{}heartbeat=30",
                if url.contains('?') { '&' } else { '?' }
            )
        };
        Self {
            url,
            consumidor: consumidor.to_string(),
            estado: Arc::new(Mutex::new(Estado {
                canal: None,
                conectado: false,
            })),
            consumidores: Mutex::new(HashMap::new()),
            colas: Mutex::new(Vec::new()),
        }
    }

    pub async fn conectar(&self) -> Result<(), lapin::Error> {
        let conexion = Connection::connect(&self.url, ConnectionProperties::default()).await?;
        let canal = conexion.create_channel().await?;
        for exchange in [EXCHANGE_EVENTS, EXCHANGE_DLQ] {
            canal
                .exchange_declare(
                    exchange,
                    ExchangeKind::Topic,
                    ExchangeDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await?;
        }
        {
            let mut estado = self.estado.lock().await;
            estado.canal = Some(canal);
            estado.conectado = true;
        }
        self.redeclarar().await?;
        self.iniciar_consumidores().await;
        logs::info(
            "rabbitmq",
            campos! { "msg": "Broker conectado", "exchange": EXCHANGE_EVENTS },
        );
        Ok(())
    }

    pub async fn declarar_colas(&self, colas: Vec<ColaConfig>) -> Result<(), String> {
        {
            let mut guardadas = self.colas.lock().await;
            guardadas.extend(colas);
        }
        self.redeclarar().await.map_err(|error| error.to_string())
    }

    pub async fn consumir(&self, cola: &str, handler: Handler) {
        {
            let mut consumidores = self.consumidores.lock().await;
            consumidores.insert(cola.to_string(), handler.clone());
        }
        let canal = self.canal().await;
        if let Some(canal) = canal {
            self.iniciar_consumo(canal, cola.to_string(), handler);
        }
    }

    /// Publica un evento con confirmación. `false` si el canal no está listo.
    pub async fn publicar(&self, evento: &Value) -> bool {
        let Some(canal) = self.canal().await else {
            return false;
        };
        let Some(tipo) = evento.get("tipo").and_then(Value::as_str) else {
            return false;
        };
        let mut props = BasicProperties::default()
            .with_delivery_mode(2)
            .with_content_type("application/json".into());
        if let Some(request_id) = evento.get("request_id").and_then(Value::as_str) {
            let mut headers = FieldTable::default();
            headers.insert(
                ShortString::from("x-request-id"),
                AMQPValue::LongString(request_id.into()),
            );
            props = props.with_headers(headers);
        }
        match canal
            .basic_publish(
                EXCHANGE_EVENTS,
                tipo,
                BasicPublishOptions::default(),
                evento.to_string().as_bytes(),
                props,
            )
            .await
        {
            Ok(_) => canal.wait_for_confirms().await.is_ok(),
            Err(error) => {
                logs::error(
                    "rabbitmq",
                    campos! { "msg": "Fallo al publicar evento", "err": error.to_string() },
                );
                false
            }
        }
    }

    /// Poller de reintento de DLQ (cada 10 s en el arranque real).
    pub fn activar_reintento(self: &Arc<Self>, intervalo: Duration) {
        let rabbit = Arc::clone(self);
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(intervalo);
            ticker.tick().await; // setInterval: el primer ciclo es al intervalo
            logs::info(
                "rabbitmq",
                campos! {
                    "msg": "Reintento automatico DLQ activo",
                    "intervalo_ms": intervalo.as_millis() as u64,
                },
            );
            loop {
                ticker.tick().await;
                let colas = rabbit.colas.lock().await.clone();
                for cola in colas {
                    if let Err(error) = rabbit.reintentar_desde_dlq(&cola.nombre).await {
                        logs::error(
                            "rabbitmq",
                            campos! {
                                "msg": "Fallo el poller de reintento DLQ",
                                "cola": cola.nombre,
                                "err": error,
                            },
                        );
                    }
                }
            }
        });
    }

    /// Supervisor de reconexión con backoff (1 s → 30 s).
    pub fn iniciar_supervisor(self: &Arc<Self>) {
        let rabbit = Arc::clone(self);
        tokio::spawn(async move {
            let mut intentos: u32 = 0;
            loop {
                tokio::time::sleep(Duration::from_secs(5)).await;
                let conectado = rabbit.estado.lock().await.conectado;
                if conectado {
                    intentos = 0;
                    continue;
                }
                let espera = std::cmp::min(
                    RECONEXION_MAX_MS,
                    RECONEXION_BASE_MS * 2u64.pow(intentos.min(5)),
                );
                tokio::time::sleep(Duration::from_millis(espera)).await;
                if let Err(error) = rabbit.conectar().await {
                    intentos += 1;
                    logs::warn(
                        "rabbitmq",
                        campos! {
                            "msg": "Fallo al reconectar con RabbitMQ",
                            "err": error.to_string(),
                            "espera_ms": espera,
                        },
                    );
                }
            }
        });
    }

    pub async fn cerrar(&self) {
        let mut estado = self.estado.lock().await;
        estado.conectado = false;
        estado.canal = None;
    }

    async fn canal(&self) -> Option<Channel> {
        self.estado.lock().await.canal.clone()
    }

    async fn redeclarar(&self) -> Result<(), lapin::Error> {
        let Some(canal) = self.canal().await else {
            return Ok(());
        };
        let colas = self.colas.lock().await.clone();
        for cola in colas {
            let dlq = format!("{}.dlq", cola.nombre);
            let mut args = FieldTable::default();
            args.insert(
                ShortString::from("x-dead-letter-exchange"),
                AMQPValue::LongString(EXCHANGE_DLQ.into()),
            );
            args.insert(
                ShortString::from("x-dead-letter-routing-key"),
                AMQPValue::LongString(cola.nombre.clone().into()),
            );
            canal
                .queue_declare(
                    &cola.nombre,
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    args,
                )
                .await?;
            canal
                .queue_declare(
                    &dlq,
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await?;
            for routing in &cola.routing_keys {
                canal
                    .queue_bind(
                        &cola.nombre,
                        EXCHANGE_EVENTS,
                        routing,
                        QueueBindOptions::default(),
                        FieldTable::default(),
                    )
                    .await?;
            }
            canal
                .queue_bind(
                    &dlq,
                    EXCHANGE_DLQ,
                    &cola.nombre,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await?;
        }
        Ok(())
    }

    async fn iniciar_consumidores(&self) {
        let Some(canal) = self.canal().await else {
            return;
        };
        let consumidores = self.consumidores.lock().await.clone();
        for (cola, handler) in consumidores {
            self.iniciar_consumo(canal.clone(), cola, handler);
        }
    }

    fn iniciar_consumo(&self, canal: Channel, cola: String, handler: Handler) {
        let consumidor = self.consumidor.clone();
        let estado = Arc::clone(&self.estado);
        tokio::spawn(async move {
            if let Err(error) = canal.basic_qos(1, BasicQosOptions::default()).await {
                logs::error(
                    "rabbitmq",
                    campos! { "msg": "Fallo al fijar prefetch", "err": error.to_string() },
                );
                return;
            }
            let mut consumidor_amqp = match canal
                .basic_consume(
                    &cola,
                    &consumidor,
                    BasicConsumeOptions::default(),
                    FieldTable::default(),
                )
                .await
            {
                Ok(consumidor) => consumidor,
                Err(error) => {
                    logs::error(
                        "rabbitmq",
                        campos! {
                            "msg": "No se pudo consumir la cola",
                            "cola": cola,
                            "err": error.to_string(),
                        },
                    );
                    marcar_desconectado(&estado).await;
                    return;
                }
            };

            while let Some(entrega) = consumidor_amqp.next().await {
                let Ok(entrega) = entrega else {
                    break;
                };
                let intentos = leer_intentos(&entrega);
                let cuerpo: Value = match serde_json::from_slice(entrega.data.as_slice()) {
                    Ok(cuerpo) => cuerpo,
                    Err(error) => {
                        let mensaje = format!("JSON invalido: {error}");
                        let _ = dead_letter(
                            &canal,
                            &cola,
                            &entrega,
                            REINTENTOS_MAXIMOS,
                            &mensaje,
                            true,
                        )
                        .await;
                        continue;
                    }
                };
                match handler(cuerpo).await {
                    Ok(()) => {
                        let _ = entrega.ack(BasicAckOptions::default()).await;
                    }
                    Err(error) => {
                        let siguiente = intentos + 1;
                        let _ = dead_letter(
                            &canal,
                            &cola,
                            &entrega,
                            siguiente,
                            &error,
                            siguiente > REINTENTOS_MAXIMOS,
                        )
                        .await;
                    }
                }
            }
            marcar_desconectado(&estado).await;
        });
    }

    async fn reintentar_desde_dlq(&self, cola: &str) -> Result<usize, String> {
        let Some(canal) = self.canal().await else {
            return Ok(0);
        };
        let dlq = format!("{cola}.dlq");
        let mut reprocesados = 0;
        loop {
            let mensaje = match canal
                .basic_get(&dlq, BasicGetOptions { no_ack: false })
                .await
            {
                Ok(mensaje) => mensaje,
                Err(error) => return Err(error.to_string()),
            };
            let Some(mensaje) = mensaje else {
                break;
            };
            let entrega = &mensaje.delivery;
            let intentos = leer_intentos(entrega);
            if intentos >= REINTENTOS_MAXIMOS {
                let _ = entrega.ack(BasicAckOptions::default()).await;
                continue;
            }
            let espera = std::cmp::min(30_000, BACKOFF_BASE_MS * 2u64.pow(intentos.min(10)));
            tokio::time::sleep(Duration::from_millis(espera)).await;

            let routing = leer_cabecera_texto(entrega, "x-routing-key-original")
                .unwrap_or_else(|| entrega.routing_key.to_string());
            let mut headers = entrega.properties.headers().clone().unwrap_or_default();
            headers.insert(ShortString::from("x-reintentado"), AMQPValue::Boolean(true));
            let props = BasicProperties::default()
                .with_delivery_mode(2)
                .with_headers(headers);
            match canal
                .basic_publish(
                    EXCHANGE_EVENTS,
                    &routing,
                    BasicPublishOptions::default(),
                    &entrega.data,
                    props,
                )
                .await
            {
                Ok(_) if canal.wait_for_confirms().await.is_ok() => {
                    let _ = entrega.ack(BasicAckOptions::default()).await;
                    reprocesados += 1;
                }
                _ => {
                    let _ = entrega
                        .nack(BasicNackOptions {
                            requeue: true,
                            multiple: false,
                        })
                        .await;
                    break;
                }
            }
        }
        Ok(reprocesados)
    }
}

/// Marca el canal como caído al terminar el stream (dispara el supervisor).
async fn marcar_desconectado(estado: &Arc<Mutex<Estado>>) {
    let mut estado = estado.lock().await;
    estado.conectado = false;
    estado.canal = None;
}

async fn dead_letter(
    canal: &Channel,
    cola: &str,
    entrega: &Delivery,
    intentos: u32,
    error: &str,
    muerto: bool,
) {
    let mut headers = entrega.properties.headers().clone().unwrap_or_default();
    headers.insert(
        ShortString::from("x-intentos"),
        AMQPValue::LongUInt(intentos),
    );
    headers.insert(
        ShortString::from("x-error"),
        AMQPValue::LongString(error.into()),
    );
    headers.insert(
        ShortString::from("x-cola"),
        AMQPValue::LongString(cola.into()),
    );
    headers.insert(
        ShortString::from("x-routing-key-original"),
        AMQPValue::LongString(entrega.routing_key.to_string().into()),
    );
    if muerto {
        headers.insert(ShortString::from("x-muerto"), AMQPValue::Boolean(true));
    }
    let props = BasicProperties::default()
        .with_delivery_mode(2)
        .with_headers(headers);
    let publicado = canal
        .basic_publish(
            EXCHANGE_DLQ,
            cola,
            BasicPublishOptions::default(),
            &entrega.data,
            props,
        )
        .await
        .is_ok()
        && canal.wait_for_confirms().await.is_ok();
    if publicado {
        let _ = entrega.ack(BasicAckOptions::default()).await;
    } else {
        let _ = entrega
            .nack(BasicNackOptions {
                requeue: !muerto,
                multiple: false,
            })
            .await;
    }
}

fn leer_intentos(entrega: &Delivery) -> u32 {
    let Some(headers) = entrega.properties.headers() else {
        return 0;
    };
    headers
        .inner()
        .get("x-intentos")
        .map(|valor| match valor {
            AMQPValue::LongUInt(numero) => *numero,
            AMQPValue::LongInt(numero) => (*numero).max(0) as u32,
            AMQPValue::LongLongInt(numero) => (*numero).max(0) as u32,
            AMQPValue::ShortUInt(numero) => *numero as u32,
            _ => 0,
        })
        .unwrap_or(0)
}

fn leer_cabecera_texto(entrega: &Delivery, clave: &str) -> Option<String> {
    let headers = entrega.properties.headers().as_ref()?;
    match headers.inner().get(clave) {
        Some(AMQPValue::LongString(valor)) => Some(valor.to_string()),
        Some(AMQPValue::ShortString(valor)) => Some(valor.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_normalizar_el_heartbeat_de_la_url() {
        let rabbit = Rabbit::nueva("amqp://guest:guest@localhost:5672", "notifications-service");
        assert!(rabbit.url.ends_with("?heartbeat=30"));
        let rabbit = Rabbit::nueva("amqp://localhost:5672?vhost=/core", "notifications-service");
        assert!(rabbit.url.ends_with("&heartbeat=30"));
        let rabbit = Rabbit::nueva(
            "amqp://localhost:5672?heartbeat=15",
            "notifications-service",
        );
        assert_eq!(rabbit.url, "amqp://localhost:5672?heartbeat=15");
    }

    #[test]
    fn debe_respetar_los_limites_de_reintento() {
        assert_eq!(REINTENTOS_MAXIMOS, 3);
        assert_eq!(BACKOFF_BASE_MS, 500);
    }

    #[test]
    fn debe_conservar_el_nombre_del_consumidor() {
        let rabbit = Rabbit::nueva("amqp://localhost", "notifications-service");
        assert_eq!(rabbit.consumidor, "notifications-service");
    }
}
