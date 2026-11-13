//! Dominio de identidad. Cada módulo replica un servicio NestJS y concentra
//! su SQL; los handlers HTTP solo validan y traducen.

pub mod auth;
pub mod capacidades;
pub mod packs;
pub mod password_reset;
pub mod sesiones;
pub mod usuarios;
