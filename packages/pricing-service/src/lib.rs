//! `pricing-service` — motor de precios (ADR-0018).
//!
//! Listas de precio, escalas, tarifas, cotizaciones, histórico y promociones.
//! Sustituye la superficie de precios heredada sin cambiar el
//! contrato HTTP ni la resolución que consumen orders y stores.

#![allow(dead_code)]

pub mod auth;
pub mod config;
pub mod db;
pub mod errors;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod money;
pub mod outbox;
pub mod paginacion;
pub mod precios;
pub mod promociones;
pub mod rabbit;
pub mod state;
pub mod tarifas;
