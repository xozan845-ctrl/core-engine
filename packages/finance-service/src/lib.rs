//! `finance-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0012-finance-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

pub mod auth;
pub mod comercio;
pub mod config;
pub mod consumidor;
pub mod contabilidad;
pub mod db;
pub mod errors;
pub mod facturacion;
pub mod finanzas;
pub mod http;
pub mod logs;
pub mod metricas;
pub mod metrics;
pub mod money;
pub mod outbox;
pub mod rabbit;
pub mod rutas;
pub mod state;
pub mod tributacion;

pub use rutas::registrar;
