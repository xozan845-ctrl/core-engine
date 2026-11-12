//! Tipos externos mínimos que necesita una notificación (ADR-0003).
//!
//! Solo se modelan los campos observables por las plantillas; cualquier campo
//! adicional de las APIs internas se ignora al deserializar.

use serde::Deserialize;

/// Orden mínima que necesita una notificación (vista interna de orders).
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct OrdenExterna {
    pub id: String,
    pub comprador_id: String,
    pub vendedor_id: String,
    pub estado: String,
    pub total_cents: i64,
    pub moneda: String,
    #[serde(default)]
    pub items: Option<Vec<ItemOrdenExterno>>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ItemOrdenExterno {
    #[serde(default)]
    pub producto_nombre: Option<String>,
    #[serde(default)]
    pub cantidad: Option<i64>,
}

/// Usuario mínimo que necesita una notificación (API interna de identity).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct UsuarioExterno {
    pub id: String,
    pub nombre: String,
    pub correo: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destinatario {
    Comprador,
    Vendedor,
}

impl Destinatario {
    pub fn como_str(self) -> &'static str {
        match self {
            Self::Comprador => "comprador",
            Self::Vendedor => "vendedor",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MensajeNotificacion {
    pub destinatario: Destinatario,
    pub mensaje: crate::email::EmailMessage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextoPedido {
    pub orden: OrdenExterna,
    pub comprador: Option<UsuarioExterno>,
    pub vendedor: Option<UsuarioExterno>,
}
