//! `storage-service` en Rust — reescritura con paridad de contrato (ADR-0011).

pub mod almacen;
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
pub mod storage;
