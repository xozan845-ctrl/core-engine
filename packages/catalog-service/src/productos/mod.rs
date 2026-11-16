//! Catálogo maestro **por empresa** (ADR-27) con variantes e imágenes (ADR-28).
//!
//! Réplica de `productos.service.ts`: un producto es un item `tipo='producto'`,
//! una variante es un item hijo (`parent_item_id`) y las imágenes viven en
//! `catalog.item_imagenes`. Las reservas/órdenes siguen operando por
//! `(empresa, código)` y el padre agrega stock y precio mínimo.
//!
//! La entrada y la salida pública son `serde_json::Value` para conservar el
//! contrato exacto de la API y del consumidor de pedidos.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::items::Items;
use crate::money::Money;
use crate::outbox::Outbox;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction};
use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub const EMPRESA_LEGACY_ID: &str = "00000000-0000-0000-0000-000000000000";
const MAX_IMAGENES: i64 = 3;
const MAX_VARIANTES: usize = 50;
const URL_IMAGEN_PUBLICA: &str = "/api/v2/storage/public/objetos";

/// Columnas de lectura compartidas por listado, detalle y variantes.
const COLUMNAS_PRODUCTO: &str = "i.id, i.empresa_id, i.codigo AS sku, i.nombre, i.descripcion,
  i.categoria, i.precio_cents, i.precio_cents AS precio_base_cents, i.costo_cents,
  i.marca, i.codigo_barras, i.unidad, i.permite_fracciones, i.unidad_compra,
  i.contenido_cantidad, i.contenido_unidad, i.tipo_precio, i.tipo_costo,
  i.stock_minimo, i.stock_maximo, i.garantia_meses, i.es_materia_prima, i.ubicacion,
  i.actualizado_en, i.stock, i.estado, i.creado_en,
  i.parent_item_id AS producto_padre_id, i.atributos,
  COALESCE((SELECT MIN(c.precio_cents) FROM catalog.items c
             WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL),
           i.precio_cents) AS precio_desde_cents,
  COALESCE((SELECT SUM(c.stock) FROM catalog.items c
             WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL),
           i.stock) AS stock_total";

/// Existencia de variantes vendibles del item.
const EXISTE_VARIANTES: &str = "EXISTS (SELECT 1 FROM catalog.items c
             WHERE c.parent_item_id = i.id AND c.eliminado_en IS NULL) AS tiene_variantes";

#[derive(Debug, Clone, PartialEq)]
struct Metadatos {
    costo_cents: i64,
    marca: String,
    codigo_barras: String,
    unidad: String,
    permite_fracciones: bool,
    unidad_compra: String,
    contenido_cantidad: i64,
    contenido_unidad: String,
    tipo_precio: String,
    tipo_costo: String,
    stock_minimo: i64,
    stock_maximo: Option<i64>,
    garantia_meses: Option<i64>,
    es_materia_prima: bool,
    ubicacion: String,
    estado_operativo: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ImagenInput {
    objeto_id: String,
    orden: Option<i64>,
    es_principal: bool,
}

#[derive(Debug, Clone)]
struct VarianteInput {
    sku: String,
    nombre: Option<String>,
    atributos: Value,
    precio_base: Option<Value>,
    stock: Option<Value>,
    imagenes: Vec<ImagenInput>,
}

/// Línea de reserva/reintegro. `item_id` solo viaja cuando el llamador lo
/// conoce (permite expandir la composición BOM).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Linea {
    #[serde(default)]
    empresa_id: String,
    #[serde(default)]
    sku: String,
    #[serde(default)]
    cantidad: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    item_id: Option<String>,
}

/// Parámetro heterogéneo para el UPDATE dinámico de `actualizar`.
enum Param {
    Texto(String),
    Entero(i64),
    OpcionalEntero(Option<i64>),
    Booleano(bool),
    Json(String),
}

pub struct Productos {
    db: Arc<Db>,
    outbox: Option<Arc<Outbox>>,
    items: Option<Arc<Items>>,
    identidad_url: String,
    storage_url: String,
    stores_url: String,
    clave_interna: String,
    timeout_identidad_ms: u64,
}

impl Productos {
    /// `outbox` e `items` son opcionales igual que en NestJS (`@Optional`):
    /// sin outbox no se emite `stock.updated` al ajustar stock a mano y sin
    /// items no se validan los atributos contra JSON Schema.
    pub fn nueva(db: Arc<Db>, outbox: Option<Arc<Outbox>>, items: Option<Arc<Items>>) -> Self {
        Self {
            db,
            outbox,
            items,
            identidad_url: variable("IDENTITY_SERVICE_URL", "http://identity-service:3001"),
            storage_url: variable("STORAGE_SERVICE_URL", "http://storage-service:3010"),
            stores_url: variable("STORES_SERVICE_URL", "http://stores-service:3003"),
            clave_interna: env::var("INTERNAL_API_KEY").unwrap_or_default(),
            timeout_identidad_ms: env::var("INTERNAL_REQUEST_TIMEOUT_MS")
                .ok()
                .and_then(|valor| valor.trim().parse::<u64>().ok())
                .filter(|valor| *valor > 0)
                .unwrap_or(3_000),
        }
    }
}

mod consultas;
mod escritura;
mod helpers;
mod idempotencia;
mod imagenes;
mod inventario;
mod variantes;

use helpers::*;

#[cfg(test)]
mod tests;
