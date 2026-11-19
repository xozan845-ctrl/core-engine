//! `inventory-service` — inventario y almacenes (ADR-0018).
//!
//! Primer corte del catálogo: stock, lotes FEFO, ubicaciones, movimientos,
//! ajustes, reservas y configuración de inventario. El contrato HTTP y los
//! eventos de la saga se incorporan en los PRs 3-5 de la migración.

#![allow(dead_code)]

pub mod auth;
pub mod config;
pub mod consumidor;
pub mod db;
pub mod errors;
pub mod http;
pub mod inventario;
pub mod lifecycle;
pub mod logs;
pub mod metrics;
pub mod money;
pub mod movimientos;
pub mod outbox;
pub mod paginacion;
pub mod rabbit;
pub mod reservas;
pub mod state;
pub mod stock;
pub mod ubicaciones;
