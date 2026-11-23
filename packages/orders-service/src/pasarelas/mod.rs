//! Adaptadores concretos de pasarelas detrás de `ProveedorPagos`.
//!
//! La selección es por configuración (`orders.proveedores_pago.tipo`), no por
//! código de negocio ni por variables hardcodeadas.

pub mod pagadito;
pub mod tilopay;
