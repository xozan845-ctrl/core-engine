//! Dominio de identidad. Cada módulo replica un servicio NestJS y concentra
//! su SQL; los handlers HTTP solo validan y traducen.
pub mod api_keys;
pub mod auth;
pub mod capacidades;
pub mod configuracion_sitio;
pub mod empresas;
pub mod media;
pub mod packs;
pub mod password_reset;
pub mod sesiones;
pub mod slug;
pub mod tema;
pub mod usuarios;
