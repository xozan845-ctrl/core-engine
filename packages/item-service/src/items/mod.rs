//! Items unificados del catálogo: producto, servicio y alquiler.
//!
//! La tabla `item.items` es la fuente; `item.items_vista` añade
//! `item_id`. Atributos, variantes e imágenes viven aquí.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::money::Money;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use crate::proyecciones::Proyecciones;
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;

pub const TIPOS: [&str; 3] = ["producto", "servicio", "alquiler"];

const COLUMNAS: &str = "i.id AS item_id, i.empresa_id, i.negocio_id, i.tipo, i.codigo, i.nombre,
  i.descripcion, i.categoria, i.costo_cents, i.track_inventory,
  i.track_serial, i.duration_minutes, i.atributos, i.parent_item_id,
  i.marca, i.codigo_barras, i.unidad, i.permite_fracciones, i.unidad_compra,
  i.contenido_cantidad, i.contenido_unidad, i.tipo_precio, i.tipo_costo,
  i.stock_minimo, i.stock_maximo, i.garantia_meses, i.es_materia_prima,
  i.ubicacion, i.estado, i.creado_en, i.actualizado_en";

const COLUMNAS_RETURNING: &str =
    "id AS item_id, empresa_id, negocio_id, tipo, codigo, nombre, descripcion,
  categoria, costo_cents, track_inventory, track_serial,
  duration_minutes, atributos, parent_item_id, marca, codigo_barras, unidad,
  permite_fracciones, unidad_compra, contenido_cantidad, contenido_unidad,
  tipo_precio, tipo_costo, stock_minimo, stock_maximo, garantia_meses,
  es_materia_prima, ubicacion, estado, creado_en, actualizado_en";

pub struct Items {
    db: Arc<Db>,
    proyecciones: Option<Arc<Proyecciones>>,
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn texto_opcional<'a>(valor: &'a Value, clave: &str) -> Option<&'a str> {
    valor.get(clave).and_then(Value::as_str).map(str::trim)
}

fn entero(valor: &Value, clave: &str) -> Option<i64> {
    valor.get(clave).and_then(Value::as_i64)
}

fn booleano(valor: &Value, clave: &str) -> Option<bool> {
    valor.get(clave).and_then(Value::as_bool)
}

fn monto(valor: &Value, clave: &str) -> Result<Money, ApiError> {
    match valor.get(clave) {
        Some(Value::String(texto)) => Money::parsear(texto),
        Some(Value::Number(numero)) => Money::parsear(&numero.to_string()),
        _ => Err(ApiError::dominio(
            "MONTO_INVALIDO",
            "Formato de monto invalido: \"undefined\". Use numero con hasta 2 decimales.",
        )),
    }
}

fn es_uuid(valor: &str) -> bool {
    Uuid::parse_str(valor).is_ok()
}

/// Valida un esquema JSON Schema (AJV `strict:false`).
pub fn compilar_esquema(esquema: &Value) -> Result<(), String> {
    jsonschema::validator_for(esquema)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Valida atributos contra un JSON Schema y devuelve los errores con el shape
/// `{instancia, parametro, mensaje}` del contrato.
pub fn errores_esquema(esquema: &Value, atributos: &Value) -> Vec<Value> {
    match jsonschema::validator_for(esquema) {
        Ok(validador) => validador
            .iter_errors(atributos)
            .map(|error| {
                json!({
                    "instancia": error.instance_path().to_string(),
                    "parametro": {},
                    "mensaje": error.to_string(),
                })
            })
            .collect(),
        Err(error) => vec![json!({
            "instancia": "",
            "parametro": {},
            "mensaje": error.to_string(),
        })],
    }
}

mod consultas;
mod escritura;

impl Items {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self {
            db,
            proyecciones: None,
        }
    }

    pub fn con_proyecciones(db: Arc<Db>, proyecciones: Arc<Proyecciones>) -> Self {
        Self {
            db,
            proyecciones: Some(proyecciones),
        }
    }

    pub fn proyecciones(&self) -> Option<&Proyecciones> {
        self.proyecciones.as_deref()
    }
}

fn la_empresa(valor: Option<&Value>) -> Result<String, ApiError> {
    let empresa = valor.and_then(Value::as_str).map(str::trim).unwrap_or("");
    if !es_uuid(empresa) {
        return Err(ApiError::dominio(
            "EMPRESA_REQUERIDA",
            "Indica una empresa válida.",
        ));
    }
    Ok(empresa.to_string())
}

pub fn exigir_tipo(valor: &Value) -> Result<&str, ApiError> {
    valor
        .as_str()
        .filter(|tipo| TIPOS.contains(tipo))
        .ok_or_else(|| {
            ApiError::dominio(
                "TIPO_ITEM_INVALIDO",
                "El tipo debe ser producto, servicio o alquiler.",
            )
        })
}

fn normalizar_codigo(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_uppercase();
    let valido = (2..=32).contains(&codigo.len())
        && codigo.chars().all(|caracter| {
            caracter.is_ascii_uppercase() || caracter.is_ascii_digit() || caracter == '-'
        });
    if !valido {
        return Err(ApiError::dominio(
            "CODIGO_INVALIDO",
            "El código solo admite letras, números y guiones (2-32).",
        ));
    }
    Ok(codigo)
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

fn es_violacion_unicidad(error: &sqlx::Error) -> bool {
    match error {
        sqlx::Error::Database(db) => db.code().as_deref() == Some("23505"),
        _ => false,
    }
}

/// Serializa una fila de `items_vista` en el orden del contrato `Item`.
pub fn serializar(fila: &Value) -> Result<Value, ApiError> {
    let texto = |clave: &str| {
        fila.get(clave)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let opcional = |clave: &str| fila.get(clave).cloned().unwrap_or(Value::Null);
    let entero = |clave: &str| fila.get(clave).and_then(Value::as_i64);
    let mut objeto = Map::new();
    objeto.insert("item_id".into(), Value::String(texto("item_id")));
    objeto.insert("empresa_id".into(), Value::String(texto("empresa_id")));
    objeto.insert("tipo".into(), Value::String(texto("tipo")));
    objeto.insert("codigo".into(), Value::String(texto("codigo")));
    objeto.insert("nombre".into(), Value::String(texto("nombre")));
    objeto.insert("descripcion".into(), Value::String(texto("descripcion")));
    objeto.insert("categoria".into(), Value::String(texto("categoria")));
    objeto.insert(
        "costo".into(),
        Value::String(Money::desde_centavos(entero("costo_cents").unwrap_or(0))?.string()),
    );
    objeto.insert(
        "track_inventory".into(),
        Value::Bool(
            fila.get("track_inventory")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert(
        "track_serial".into(),
        Value::Bool(
            fila.get("track_serial")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert("duration_minutes".into(), opcional("duration_minutes"));
    objeto.insert(
        "atributos".into(),
        fila.get("atributos").cloned().unwrap_or_else(|| json!({})),
    );
    objeto.insert("parent_item_id".into(), opcional("parent_item_id"));
    objeto.insert(
        "marca".into(),
        Value::String(
            fila.get("marca")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
    );
    objeto.insert(
        "codigo_barras".into(),
        Value::String(
            fila.get("codigo_barras")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
    );
    objeto.insert(
        "unidad".into(),
        Value::String(
            fila.get("unidad")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .unwrap_or("Unidad (u)")
                .to_string(),
        ),
    );
    objeto.insert(
        "permite_fracciones".into(),
        Value::Bool(
            fila.get("permite_fracciones")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert(
        "unidad_compra".into(),
        Value::String(
            fila.get("unidad_compra")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .unwrap_or("Igual que la de venta")
                .to_string(),
        ),
    );
    objeto.insert(
        "contenido_cantidad".into(),
        Value::from(entero("contenido_cantidad").unwrap_or(0)),
    );
    objeto.insert(
        "contenido_unidad".into(),
        Value::String(
            fila.get("contenido_unidad")
                .and_then(Value::as_str)
                .filter(|valor| !valor.is_empty())
                .unwrap_or("Sin contenido")
                .to_string(),
        ),
    );
    objeto.insert(
        "tipo_precio".into(),
        Value::String(
            if texto("tipo_precio") == "escala" {
                "escala"
            } else {
                "fijo"
            }
            .to_string(),
        ),
    );
    objeto.insert(
        "tipo_costo".into(),
        Value::String(
            if texto("tipo_costo") == "escala" {
                "escala"
            } else {
                "fijo"
            }
            .to_string(),
        ),
    );
    objeto.insert(
        "stock_minimo".into(),
        Value::from(entero("stock_minimo").unwrap_or(0)),
    );
    objeto.insert("stock_maximo".into(), opcional("stock_maximo"));
    objeto.insert("garantia_meses".into(), opcional("garantia_meses"));
    objeto.insert(
        "es_materia_prima".into(),
        Value::Bool(
            fila.get("es_materia_prima")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        ),
    );
    objeto.insert(
        "ubicacion".into(),
        Value::String(
            fila.get("ubicacion")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        ),
    );
    objeto.insert("imagenes".into(), json!([]));
    objeto.insert(
        "estado".into(),
        Value::String(
            if texto("estado") == "inactivo" {
                "inactivo"
            } else {
                "activo"
            }
            .to_string(),
        ),
    );
    objeto.insert("creado_en".into(), Value::String(texto("creado_en")));
    objeto.insert(
        "actualizado_en".into(),
        Value::String(texto("actualizado_en")),
    );
    Ok(Value::Object(objeto))
}

/// Transacción con claims de RLS (reutilizada por módulos hermanos).
pub async fn transaccion(items: &Items) -> Result<Transaction<'static, Postgres>, ApiError> {
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn debe_normalizar_y_validar_codigos_y_nombres() {
            assert_eq!(normalizar_codigo(" sku-1 ").expect("codigo"), "SKU-1");
            assert_eq!(
                normalizar_codigo("a").expect_err("corto").codigo,
                "CODIGO_INVALIDO"
            );
            assert_eq!(
                normalizar_codigo("skú").expect_err("simbolo").codigo,
                "CODIGO_INVALIDO"
            );
            assert_eq!(validar_nombre(" Producto ").expect("nombre"), "Producto");
            assert_eq!(
                validar_nombre("x").expect_err("corto").codigo,
                "NOMBRE_INVALIDO"
            );
        }

        #[test]
        fn debe_validar_esquemas_json() {
            assert!(compilar_esquema(&json!({
                "type": "object",
                "properties": {"color": {"type": "string"}},
                "required": ["color"],
                "additionalProperties": false
            }))
            .is_ok());
            let errores = errores_esquema(
                &json!({
                    "type": "object",
                    "properties": {"color": {"type": "string"}},
                    "required": ["color"],
                    "additionalProperties": false
                }),
                &json!({"otro": 1}),
            );
            assert!(!errores.is_empty());
            assert!(errores[0].get("instancia").is_some());
        }

        #[test]
        fn debe_serializar_item_con_montos_y_defaults() {
            let fila = json!({
                "item_id": "11111111-1111-4111-8111-111111111111",
                "empresa_id": "22222222-2222-4222-8222-222222222222",
                "tipo": "producto",
                "codigo": "SKU-1",
                "nombre": "Producto",
                "descripcion": "",
                "categoria": "general",
                "precio_cents": 115000,
                "costo_cents": 0,
                "track_inventory": true,
                "track_serial": false,
                "stock": 5,
                "duration_minutes": null,
                "atributos": {},
                "parent_item_id": null,
                "marca": null,
                "codigo_barras": null,
                "unidad": null,
                "permite_fracciones": null,
                "unidad_compra": null,
                "contenido_cantidad": null,
                "contenido_unidad": null,
                "tipo_precio": null,
                "tipo_costo": null,
                "stock_minimo": null,
                "stock_maximo": null,
                "garantia_meses": null,
                "es_materia_prima": null,
                "ubicacion": null,
                "estado": "activo",
                "creado_en": "2026-11-13T00:00:00.000Z",
                "actualizado_en": "2026-11-13T00:00:00.000Z"
            });
            let item = serializar(&fila).expect("item");
            assert!(item.get("precio").is_none());
            assert!(item.get("stock").is_none());
            assert_eq!(item["unidad"], "Unidad (u)");
            assert_eq!(item["tipo_precio"], "fijo");
            assert_eq!(item["imagenes"], json!([]));
        }
    }
    items.db.transaccion().await.map_err(interno)
}
