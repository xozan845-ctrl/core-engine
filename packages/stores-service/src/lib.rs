//! `stores-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0011-stores-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

// El andamiaje expone piezas (errores, métricas, auth, clientes) que aún no
// tienen rutas que las consuman; el `allow` desaparece al completar el dominio.
#![allow(dead_code)]

pub mod auth;
pub mod clients;
pub mod config;
pub mod db;
pub mod errors;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod ofertas;
pub mod state;
pub mod tiendas;
