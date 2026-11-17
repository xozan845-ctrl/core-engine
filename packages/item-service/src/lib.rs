//! `item-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0016-item-service-en-rust.md`. La lógica vive en esta
//! librería para que el binario y las pruebas de integración compartan el mismo
//! código.

// El andamiaje expone piezas (errores, métricas, auth, Outbox, Rabbit) que aún
// no tienen rutas que las consuman; el `allow` desaparece al completar el
// dominio.
#![allow(dead_code)]

pub mod auth;
pub mod componentes;
pub mod config;
pub mod db;
pub mod errors;
pub mod http;
pub mod items;
pub mod logs;
pub mod metrics;
pub mod money;
pub mod operaciones;
pub mod outbox;
pub mod paginacion;
pub mod productos;
pub mod proyecciones;
pub mod rabbit;
pub mod series;
pub mod servicios;
pub mod state;
