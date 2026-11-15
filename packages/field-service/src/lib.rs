//! `field-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0008-field-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

pub mod auth;
pub mod clients;
pub mod config;
pub mod db;
pub mod errors;
pub mod field;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod state;
