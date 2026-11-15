//! `market-intelligence-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0009-market-intelligence-service-en-rust.md`.

// El andamiaje expone piezas que los tests usan y que no todas las rutas
// consumen; el `allow` desaparece al estabilizar el servicio.
#![allow(dead_code)]

pub mod agregador;
pub mod auth;
pub mod calidad;
pub mod config;
pub mod consumidor;
pub mod db;
pub mod errors;
pub mod http;
pub mod inteligencia;
pub mod logs;
pub mod metrics;
pub mod outbox;
pub mod rabbit;
pub mod state;
pub mod tareas;
