//! `commissions-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0007-commissions-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

pub mod auth;
pub mod clients;
pub mod comisiones;
pub mod config;
pub mod db;
pub mod errors;
pub mod http;
pub mod liquidaciones;
pub mod logs;
pub mod metrics;
pub mod money;
pub mod outbox;
pub mod pedidos;
pub mod rabbit;
pub mod reportes;
pub mod state;
pub mod ventas;
pub mod worker;
