//! Operaciones avanzadas del catálogo (R-PS-04).
//!
//! Réplica de `operaciones.service.ts`: importación de productos, escaneo por
//! código y combos. Las reglas de cálculo (filas válidas/omitidas, costos y
//! márgenes de combos) viven en funciones puras para poder probarlas sin base
//! de datos. Precios, promociones e inventario pertenecen a `pricing-service`
//! e `inventory-service` (ADR-0018).
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
use crate::proyecciones::Proyecciones;
use crate::servicios::Servicios;
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

const MAX_IMPORTACION: usize = 200;
const SELECT_COMBO: &str = "SELECT i.id, i.empresa_id, i.codigo AS sku, i.nombre, i.descripcion,
  i.codigo_barras, i.estado,
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
 FROM item.items i
 LEFT JOIN item.item_componentes ic ON ic.item_id = i.id
 LEFT JOIN item.items ci
   ON ci.id = ic.componente_item_id AND ci.eliminado_en IS NULL";

#[derive(Debug, Clone, PartialEq)]
struct ItemResuelto {
    id: String,
    sku: String,
    nombre: String,
    #[allow(dead_code)]
    costo_cents: i64,
    es_combo: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct ComponenteResuelto {
    item_id: String,
    sku: String,
    nombre: String,
    cantidad: i64,
}

/// Operaciones de catálogo por empresa: importación, escaneo y combos.
pub struct Operaciones {
    db: Arc<Db>,
    outbox: Option<Arc<Outbox>>,
    productos: Arc<Productos>,
    servicios: Arc<Servicios>,
    proyecciones: Option<Arc<Proyecciones>>,
}

mod combos;
mod importacion;

impl Operaciones {
    pub fn nueva(
        db: Arc<Db>,
        outbox: Option<Arc<Outbox>>,
        proyecciones: Option<Arc<Proyecciones>>,
    ) -> Self {
        let items = Arc::new(match proyecciones.clone() {
            Some(proyecciones) => Items::con_proyecciones(db.clone(), proyecciones),
            None => Items::nueva(db.clone()),
        });
        let productos = Arc::new(Productos::nueva(
            db.clone(),
            outbox.clone(),
            Some(items.clone()),
        ));
        let servicios = Arc::new(
            Servicios::con_productos(db.clone(), productos.clone(), Some(items))
                .con_proyecciones(proyecciones.clone()),
        );
        Self {
            productos,
            servicios,
            db,
            outbox,
            proyecciones,
        }
    }

    // ── importación ──────────────────────────────────────────────────────
}

fn datos_producto_importado(fila: &Value, empresa_id: &str) -> Value {
    let mut datos = Map::new();
    datos.insert("empresa_id".into(), Value::String(empresa_id.to_string()));
    for clave in [
        "sku",
        "nombre",
        "descripcion",
        "categoria",
        "precio_base",
        "stock",
        "costo",
        "marca",
        "codigo_barras",
        "unidad",
        "stock_minimo",
        "stock_maximo",
        "es_materia_prima",
        "ubicacion",
    ] {
        if let Some(valor) = fila.get(clave) {
            if !valor.is_null() {
                datos.insert(clave.to_string(), valor.clone());
            }
        }
    }
    Value::Object(datos)
}

fn cantidad_componente(componente: &Value) -> Result<i64, ApiError> {
    entero_de(componente.get("cantidad"))
        .filter(|cantidad| *cantidad >= 1)
        .ok_or_else(|| {
            ApiError::dominio(
                "CANTIDAD_INVALIDA",
                "La cantidad del componente debe ser un entero positivo.",
            )
        })
}

fn registrar_componente_unico(vistos: &mut Vec<String>, item_id: &str) -> Result<(), ApiError> {
    if vistos.iter().any(|visto| visto == item_id) {
        return Err(ApiError::dominio(
            "COMPONENTE_DUPLICADO",
            "Un producto no puede repetirse en el combo.",
        ));
    }
    vistos.push(item_id.to_string());
    Ok(())
}

fn calcular_margen(precio_cents: i64, costo_total_cents: i64) -> f64 {
    if precio_cents <= 0 {
        return 0.0;
    }
    let proporcion = (precio_cents - costo_total_cents) as f64 / precio_cents as f64;
    redondear_js(proporcion * 1000.0) / 10.0
}

fn redondear_js(valor: f64) -> f64 {
    if valor.is_sign_negative() {
        -((-valor + 0.5).floor())
    } else {
        (valor + 0.5).floor()
    }
}

fn normalizar_sku(valor: &str) -> Result<String, ApiError> {
    let sku = valor.trim().to_uppercase();
    let longitud = sku.chars().count();
    let valido = (2..=32).contains(&longitud)
        && sku.chars().all(|caracter| {
            caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '-'
        });
    if !valido {
        return Err(ApiError::dominio(
            "SKU_INVALIDO",
            "El SKU solo admite letras, números y guiones (2-32).",
        ));
    }
    Ok(sku)
}

fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim().to_string();
    let longitud = nombre.chars().count();
    if !(2..=200).contains(&longitud) {
        return Err(ApiError::dominio(
            "NOMBRE_INVALIDO",
            "El nombre debe tener entre 2 y 200 caracteres.",
        ));
    }
    Ok(nombre)
}

fn monto_de(valor: Option<&Value>) -> Result<Money, ApiError> {
    match valor {
        Some(Value::String(texto)) => Money::parsear(texto),
        Some(Value::Number(numero)) => Money::parsear(&numero.to_string()),
        Some(otro) => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            format!(
                "Formato de monto invalido: \"{}\". Use numero con hasta 2 decimales.",
                otro
            ),
        )),
        None => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        )),
    }
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn texto_opcional(valor: &Value, clave: &str) -> Option<String> {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .map(|texto| texto.trim().to_string())
}

fn filtro_texto(filtros: &Value, clave: &str) -> Option<String> {
    match filtros.get(clave) {
        Some(Value::String(valor)) => Some(valor.clone()),
        Some(Value::Number(valor)) => Some(valor.to_string()),
        _ => None,
    }
}

fn entero_de(valor: Option<&Value>) -> Option<i64> {
    match valor? {
        Value::Number(numero) => numero
            .as_i64()
            .or_else(|| numero.as_f64().and_then(f64_a_entero)),
        Value::String(texto) => {
            let texto = texto.trim();
            if texto.is_empty() {
                Some(0)
            } else {
                texto
                    .parse::<i64>()
                    .ok()
                    .or_else(|| texto.parse::<f64>().ok().and_then(f64_a_entero))
            }
        }
        _ => None,
    }
}

fn f64_a_entero(valor: f64) -> Option<i64> {
    if valor.is_finite()
        && valor.fract() == 0.0
        && valor >= i64::MIN as f64
        && valor <= i64::MAX as f64
    {
        Some(valor as i64)
    } else {
        None
    }
}

fn estado_operativo(datos: &Value) -> String {
    if datos.get("estado").and_then(Value::as_str) == Some("inactivo") {
        "inactivo".to_string()
    } else {
        "activo".to_string()
    }
}

fn estado_serializado(valor: Option<&Value>) -> String {
    if valor.and_then(Value::as_str) == Some("inactivo") {
        "inactivo".to_string()
    } else {
        "activo".to_string()
    }
}

fn actual_sku(actual: &Value) -> &str {
    actual.get("sku").and_then(Value::as_str).unwrap_or("")
}

fn es_uuid(valor: &str) -> bool {
    Uuid::parse_str(valor).is_ok()
}

fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn debe_mapear_filas_de_importacion_sin_campos_ausentes() {
            let fila = json!({
                "sku": "SKU-1",
                "nombre": "Producto",
                "precio_base": "10.00",
                "stock": 5,
                "costo": "4.50",
                "codigo_barras": "750",
                "stock_maximo": null
            });
            let datos = datos_producto_importado(&fila, "empresa-1");
            assert_eq!(datos["empresa_id"], "empresa-1");
            assert_eq!(datos["sku"], "SKU-1");
            assert_eq!(datos["precio_base"], "10.00");
            assert_eq!(datos["costo"], "4.50");
            assert!(datos.get("descripcion").is_none());
            assert!(datos.get("stock_maximo").is_none());
        }

        #[test]
        fn debe_normalizar_sku_y_validar_nombres() {
            assert_eq!(normalizar_sku(" sku-1 ").expect("sku"), "SKU-1");
            assert_eq!(
                normalizar_sku("a").expect_err("corto").codigo,
                "SKU_INVALIDO"
            );
            assert_eq!(
                normalizar_sku("skú").expect_err("simbolo").codigo,
                "SKU_INVALIDO"
            );
            assert_eq!(validar_nombre(" Producto ").expect("nombre"), "Producto");
            assert_eq!(
                validar_nombre("x").expect_err("corto").codigo,
                "NOMBRE_INVALIDO"
            );
        }

        #[test]
        fn debe_calcular_el_margen_del_combo_como_el_contrato() {
            // precio 30.00, costo 20.00 -> 33.3
            assert_eq!(calcular_margen(3000, 2000), 33.3);
            // precio 0 -> 0 sin dividir entre cero.
            assert_eq!(calcular_margen(0, 2000), 0.0);
            // Sin costo -> 100 %.
            assert_eq!(calcular_margen(3000, 0), 100.0);
            // Costo mayor que el precio: margen negativo redondeado como JS.
            assert_eq!(calcular_margen(3000, 3010), -0.3);
        }

        #[test]
        fn debe_validar_cantidades_y_duplicados_de_componentes() {
            assert_eq!(
                cantidad_componente(&json!({"cantidad": 2})).expect("cantidad"),
                2
            );
            assert_eq!(
                cantidad_componente(&json!({"cantidad": 0}))
                    .expect_err("cero")
                    .codigo,
                "CANTIDAD_INVALIDA"
            );
            assert_eq!(
                cantidad_componente(&json!({"cantidad": 1.5}))
                    .expect_err("fraccion")
                    .codigo,
                "CANTIDAD_INVALIDA"
            );

            let mut vistos = Vec::new();
            registrar_componente_unico(&mut vistos, "item-1").expect("primero");
            assert_eq!(
                registrar_componente_unico(&mut vistos, "item-1")
                    .expect_err("duplicado")
                    .codigo,
                "COMPONENTE_DUPLICADO"
            );
        }

        #[test]
        fn debe_interpretar_cantidades_como_number_de_javascript() {
            assert_eq!(entero_de(Some(&json!(5))), Some(5));
            assert_eq!(entero_de(Some(&json!(5.0))), Some(5));
            assert_eq!(entero_de(Some(&json!("5"))), Some(5));
            assert_eq!(entero_de(Some(&json!(""))), Some(0));
            assert_eq!(entero_de(Some(&json!(1.5))), None);
        }
    }
    match error {
        sqlx::Error::Database(db) => matches!(db.code().as_deref(), Some("23505") | Some("23P01")),
        _ => false,
    }
}
