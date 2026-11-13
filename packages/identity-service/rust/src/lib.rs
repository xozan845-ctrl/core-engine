//! `identity-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0017-identity-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

// El andamiaje expone piezas (auth, errores, métricas) que aún no tienen rutas
// que las consuman; el `allow` desaparece al completar los PR 2-7.
#![allow(dead_code)]

pub mod auth;
pub mod config;
pub mod db;
pub mod dominio;
pub mod email;
pub mod errors;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod state;
