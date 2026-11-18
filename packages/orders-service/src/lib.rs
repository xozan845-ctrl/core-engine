//! `orders-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0013-orders-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

// El andamiaje expone piezas (errores, métricas, auth) que aún no tienen rutas
// que las consuman; el `allow` desaparece al completar las rutas y el consumidor.
#![allow(dead_code)]

pub mod auth;
pub mod carrito;
pub mod checkout;
pub mod clients;
pub mod config;
pub mod consumer;
pub mod contratos;
pub mod db;
pub mod devoluciones;
pub mod entregas;
pub mod errors;
pub mod event_store;
pub mod fidelizacion;
pub mod fulfillments;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod money;
pub mod order_state;
pub mod outbox;
pub mod pagos;
pub mod pedidos;
pub mod rabbit;
pub mod salon;
pub mod state;
pub mod suscripciones;
pub mod tipos_orden;
pub mod views;
