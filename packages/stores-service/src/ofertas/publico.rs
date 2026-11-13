//! Proyección pública de ofertas (R-DS-3 / R-C-3).
//!
//! Lista blanca explícita: un campo nuevo no se publica por accidente.

use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
pub struct OfertaPublica {
    pub id: String,
    pub item_type: String,
    pub item_id: String,
    pub sku: String,
    pub producto_nombre: String,
    pub atributos: Value,
    pub imagen_objeto_id: Option<String>,
    pub imagen_url: Option<String>,
    pub precio_venta: String,
    pub moneda: String,
    pub stock: Value,
    pub estado: String,
    pub creado_en: String,
}

pub fn a_oferta_publica(oferta: &Value) -> OfertaPublica {
    OfertaPublica {
        id: texto(oferta, "id"),
        item_type: texto(oferta, "item_type"),
        item_id: texto(oferta, "item_id"),
        sku: texto(oferta, "sku"),
        producto_nombre: texto(oferta, "producto_nombre"),
        atributos: oferta
            .get("atributos")
            .cloned()
            .unwrap_or_else(|| json!({})),
        imagen_objeto_id: texto_opcional(oferta, "imagen_objeto_id"),
        imagen_url: texto_opcional(oferta, "imagen_url"),
        precio_venta: texto(oferta, "precio_venta"),
        moneda: texto(oferta, "moneda"),
        stock: oferta.get("stock").cloned().unwrap_or(Value::Null),
        estado: texto(oferta, "estado"),
        creado_en: texto(oferta, "creado_en"),
    }
}

fn texto(fila: &Value, clave: &str) -> String {
    fila.get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn texto_opcional(fila: &Value, clave: &str) -> Option<String> {
    fila.get(clave).and_then(Value::as_str).map(str::to_string)
}
