//! `notifications-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0006-notifications-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

#![allow(dead_code)]

pub mod clientes;
pub mod config;
pub mod consumidor;
pub mod db;
pub mod email;
pub mod errors;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod notificaciones;
pub mod plantillas;
pub mod rabbit;
pub mod state;
pub mod tipos;
