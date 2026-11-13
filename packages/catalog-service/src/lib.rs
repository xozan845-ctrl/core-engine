//! `catalog-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0016-catalog-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

// El andamiaje expone piezas (errores, métricas, auth, Outbox, Rabbit) que aún
// no tienen rutas que las consuman; el `allow` desaparece al completar el
// dominio.
#![allow(dead_code)]

pub mod auth;
pub mod config;
pub mod db;
pub mod errors;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod outbox;
pub mod rabbit;
pub mod state;
