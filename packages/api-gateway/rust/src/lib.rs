//! `api-gateway` en Rust — reescritura con paridad de contrato (ADR-0015).

pub mod auth;
pub mod config;
pub mod diagnostics;
pub mod errors;
pub mod http;
pub mod logs;
pub mod metrics;
pub mod policy;
pub mod proxy;
pub mod rate_limit;
pub mod readiness;
pub mod state;
