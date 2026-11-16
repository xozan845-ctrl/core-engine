//! `analytics-service` — único dueño de la analítica de negocio, empresa y
//! plataforma. Consume hechos de órdenes y comisiones y expone JSON/CSV.

pub mod analitica;
pub mod auth;
pub mod clients;
pub mod config;
pub mod consumidor;
pub mod db;
pub mod errors;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod rabbit;
pub mod reportes;
pub mod state;
