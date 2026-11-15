//! Ciclo de vida de la orden (Tabla 13) y contrato de línea.
//!
//! Réplica de `order-state.ts` y `ItemOrden`: mismos estados, tipos,
//! fulfillments y transiciones por tipo de orden.

use crate::errors::ApiError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const ESTADOS_ORDEN: [&str; 9] = [
    "creada",
    "pagada",
    "en_preparacion",
    "enviada",
    "aceptada",
    "rechazada",
    "entregada",
    "cancelada",
    "devuelta",
];

pub const TIPOS_ORDEN: [&str; 4] = ["compra", "reserva", "cotizacion", "orden_servicio"];
pub const FULFILLMENTS: [&str; 5] = ["delivery", "pickup", "onsite", "digital", "none"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ItemOrden {
    pub oferta_id: String,
    pub sku: String,
    pub producto_nombre: String,
    pub cantidad: i64,
    pub precio_unitario_cents: i64,
    pub vendedor_id: String,
    pub tienda_id: String,
    pub empresa_id: String,
    pub participacion_porcentaje: Option<i64>,
    pub item_type: String,
    pub item_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fulfillment_config: Option<Value>,
}

/// `true` si `de -> a` está permitido para el tipo de orden.
pub fn transicion_valida(tipo: &str, de: &str, a: &str) -> bool {
    let permitidas: &[&str] = match (tipo, de) {
        (_, "aceptada" | "rechazada" | "cancelada" | "devuelta") => &[],
        ("compra", "creada") => &["pagada", "cancelada"],
        ("compra", "pagada") => &["en_preparacion", "cancelada", "devuelta"],
        ("compra", "en_preparacion") => &["enviada", "cancelada"],
        ("compra", "enviada") => &["entregada", "devuelta"],
        ("compra", "entregada") => &["devuelta"],
        ("reserva", "creada") => &["pagada", "cancelada"],
        ("reserva", "pagada") => &["entregada", "cancelada", "devuelta"],
        ("reserva", "entregada") => &["devuelta"],
        ("cotizacion", "creada") => &["enviada", "cancelada"],
        ("cotizacion", "enviada") => &["aceptada", "rechazada", "cancelada"],
        ("orden_servicio", "creada") => &["pagada", "cancelada"],
        ("orden_servicio", "pagada") => &["en_preparacion", "cancelada", "devuelta"],
        ("orden_servicio", "en_preparacion") => &["entregada", "cancelada"],
        ("orden_servicio", "entregada") => &["devuelta"],
        _ => &[],
    };
    permitidas.contains(&a)
}

pub fn validar_transicion_por_tipo(tipo: &str, de: &str, a: &str) -> Result<(), ApiError> {
    if !TIPOS_ORDEN.contains(&tipo) {
        return Err(ApiError::dominio(
            "TIPO_ORDEN_INVALIDO",
            "El tipo de orden no es valido.",
        ));
    }
    if transicion_valida(tipo, de, a) {
        return Ok(());
    }
    Err(ApiError::dominio(
        "TRANSICION_INVALIDA",
        format!("Transicion invalida de estado \"{de}\" a \"{a}\" para orden \"{tipo}\"."),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_permitir_la_cadena_feliz_de_compra() {
        for (de, a) in [
            ("creada", "pagada"),
            ("pagada", "en_preparacion"),
            ("en_preparacion", "enviada"),
            ("enviada", "entregada"),
        ] {
            assert!(transicion_valida("compra", de, a));
        }
    }

    #[test]
    fn debe_rechazar_saltos_y_terminales() {
        assert!(!transicion_valida("compra", "creada", "entregada"));
        assert!(!transicion_valida("compra", "cancelada", "pagada"));
        assert!(!transicion_valida("compra", "pagada", "enviada"));
        assert!(!transicion_valida("compra", "creada", "creada"));
    }

    #[test]
    fn debe_permitir_el_ciclo_de_cotizacion() {
        assert!(transicion_valida("cotizacion", "creada", "enviada"));
        assert!(transicion_valida("cotizacion", "enviada", "aceptada"));
        assert!(transicion_valida("cotizacion", "enviada", "rechazada"));
        assert!(!transicion_valida("cotizacion", "creada", "aceptada"));
        assert!(!transicion_valida("cotizacion", "aceptada", "pagada"));
    }

    #[test]
    fn debe_mapear_la_transicion_invalida_al_error_de_dominio() {
        let error = validar_transicion_por_tipo("compra", "creada", "entregada")
            .expect_err("transicion invalida");
        assert_eq!(error.codigo, "TRANSICION_INVALIDA");
        assert!(error.mensaje.contains("creada"));
        assert!(error.mensaje.contains("entregada"));
    }
}
