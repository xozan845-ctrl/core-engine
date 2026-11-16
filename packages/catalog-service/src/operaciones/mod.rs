//! Operaciones avanzadas del catálogo (R-PS-04).
//!
//! Réplica de `operaciones.service.ts`: configuración de inventario,
//! importación de productos, escaneo por código, combos, promociones y
//! movimientos/traslados. Las reglas de cálculo (filas válidas/omitidas,
//! costos y márgenes de combos, vigencias y cantidades de promociones, stock
//! resultante de cada movimiento) viven en funciones puras para poder
//! probarlas sin base de datos.
//!
//! Reutiliza los módulos hermanos ya portados: `Productos::crear` (con
//! `empresa_validada`), `Productos::validar_empresa_catalogo`,
//! `Productos::encontrar_por_codigo_barras` y
//! `Servicios::encontrar_por_codigo`. Todos reciben y devuelven `Value` para
//! conservar el contrato JSON.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::items::Items;
use crate::money::Money;
use crate::outbox::Outbox;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use crate::productos::Productos;
use crate::servicios::Servicios;
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const MAX_IMPORTACION: usize = 200;
const MAX_LINEAS: usize = 200;
const UNIDAD_DEFECTO: &str = "Unidad (u)";
const EVENTO_STOCK_UPDATED: &str = "stock.updated";
const METODOS_COSTEO: [&str; 3] = ["promedio", "fifo", "ueps"];
const TIPOS_PROMOCION: [&str; 4] = [
    "2x1",
    "descuento_cantidad",
    "descuento_porcentaje",
    "regalo",
];
const TIPOS_MOVIMIENTO: [&str; 4] = ["entrada", "salida", "ajuste", "traslado"];

const COLUMNAS_CONFIGURACION: &str = "empresa_id, metodo_costeo, permitir_stock_negativo,
  alerta_stock_bajo, actualizar_costo_en_compra, unidad_defecto, actualizado_en";

const SELECT_COMBO: &str = "SELECT i.id, i.empresa_id, i.codigo AS sku, i.nombre, i.descripcion,
  i.codigo_barras, i.precio_cents, i.estado,
  COALESCE(
    jsonb_agg(
      jsonb_build_object(
        'item_id', ci.id,
        'sku', ci.codigo,
        'nombre', ci.nombre,
        'cantidad', ic.cantidad
      )
      ORDER BY ci.nombre
    ) FILTER (WHERE ic.id IS NOT NULL),
    '[]'::jsonb
  ) AS componentes,
  i.creado_en, i.actualizado_en
 FROM catalog.items i
 LEFT JOIN catalog.item_componentes ic ON ic.item_id = i.id
 LEFT JOIN catalog.items ci
   ON ci.id = ic.componente_item_id AND ci.eliminado_en IS NULL";

const COLUMNAS_PROMOCION: &str = "id, empresa_id, nombre, tipo, item_id, sku,
  cantidad_requerida, cantidad_gratis, descuento_porcentaje,
  fecha_desde::text AS fecha_desde, fecha_hasta::text AS fecha_hasta,
  estado, creado_en, actualizado_en";

const RETURNING_PROMOCION: &str = "RETURNING id, empresa_id, nombre, tipo, item_id, sku,
  cantidad_requerida, cantidad_gratis, descuento_porcentaje,
  fecha_desde::text AS fecha_desde, fecha_hasta::text AS fecha_hasta,
  estado, creado_en, actualizado_en";

const COLUMNAS_MOVIMIENTO: &str = "id, empresa_id, tipo, motivo, referencia,
  ubicacion_origen, ubicacion_destino, lineas, creado_por, creado_en";

/// Operaciones de catálogo por empresa.
pub struct Operaciones {
    db: Arc<Db>,
    outbox: Option<Arc<Outbox>>,
    productos: Arc<Productos>,
    servicios: Arc<Servicios>,
}

#[derive(Debug, Clone, PartialEq)]
struct ItemResuelto {
    id: String,
    sku: String,
    nombre: String,
    #[allow(dead_code)]
    costo_cents: i64,
    stock: i64,
    es_combo: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct ComponenteResuelto {
    item_id: String,
    sku: String,
    nombre: String,
    cantidad: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct PromocionNormalizada {
    nombre: String,
    tipo: String,
    item_id: Option<String>,
    sku: String,
    cantidad_requerida: Option<i64>,
    cantidad_gratis: Option<i64>,
    descuento_porcentaje: Option<i64>,
    fecha_desde: String,
    fecha_hasta: String,
    estado: String,
}

#[derive(Debug, Clone, PartialEq)]
struct CabeceraMovimiento {
    tipo: String,
    origen: String,
    destino: String,
}

impl Operaciones {
    pub fn nueva(db: Arc<Db>, outbox: Option<Arc<Outbox>>) -> Self {
        // Productos y servicios comparten la misma instancia de Items para que
        // la validación de atributos (JSON Schema) sea consistente.
        let items = Arc::new(Items::nueva(db.clone()));
        let productos = Arc::new(Productos::nueva(
            db.clone(),
            outbox.clone(),
            Some(items.clone()),
        ));
        let servicios = Arc::new(Servicios::con_productos(
            db.clone(),
            productos.clone(),
            Some(items),
        ));
        Self {
            productos,
            servicios,
            db,
            outbox,
        }
    }
}

mod combos;
mod configuracion;
mod helpers;
mod movimientos;
mod promociones;
mod resolver;

#[cfg(test)]
mod tests;
