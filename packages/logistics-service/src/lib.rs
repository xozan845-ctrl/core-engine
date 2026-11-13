//! `logistics-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0008-logistics-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

// El andamiaje expone piezas (errores, métricas, auth) que aún no tienen rutas
// que las consuman; el `allow` desaparece al completar las rutas y el consumidor.
#![allow(dead_code)]

pub mod auth;
pub mod clients;
pub mod config;
pub mod consumidor;
pub mod db;
pub mod envios;
pub mod errors;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod money;
pub mod outbox;
pub mod rabbit;
pub mod state;
