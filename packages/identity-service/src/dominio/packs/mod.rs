//! Packs, verticales y modelos de operación.
//!
//! Réplica de `packs/packs.service.ts`: la lectura agrupa capacidades y
//! modelos por pack; la creación valida códigos y referencias activas.

mod comunes;
mod escritura;
mod lectura;

mod prelude {
    pub(crate) use crate::db::Db;
    pub(crate) use crate::dominio::regiones::{region_de_fila, RegionTributaria};
    pub(crate) use crate::errors::ApiError;
    pub(crate) use serde::Serialize;
    pub(crate) use serde_json::{json, Value};
    pub(crate) use sqlx::Row;
    pub(crate) use std::collections::HashMap;
    pub(crate) use std::sync::Arc;
}
use self::comunes::*;
use self::prelude::*;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PackCapacidad {
    pub capacidad_codigo: String,
    pub nombre: String,
    pub modulo: String,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PackModelo {
    pub modelo_codigo: String,
    pub nombre: String,
    pub descripcion: String,
    pub activo: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pack {
    pub codigo: String,
    pub version: i64,
    pub vertical_codigo: String,
    pub nombre: String,
    pub descripcion: String,
    pub config: Value,
    pub modelos: Vec<PackModelo>,
    pub capacidades: Vec<PackCapacidad>,
    /// Regiones tributarias que el pack admite para elegir en el onboarding.
    pub regiones: Vec<RegionTributaria>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModeloOperacion {
    pub codigo: String,
    pub nombre: String,
    pub descripcion: String,
    pub config: Value,
    pub activo: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VerticalOperacion {
    pub codigo: String,
    pub nombre: String,
    pub modelo_operacion: String,
    pub activo: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrearPack {
    pub codigo: String,
    pub version: i64,
    pub vertical_codigo: String,
    pub nombre: String,
    pub descripcion: Option<String>,
    pub config: Option<Value>,
    pub modelos: Vec<PackModeloInput>,
    pub capacidades: Vec<PackCapacidadInput>,
    pub regiones: Vec<PackRegionInput>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackRegionInput {
    pub codigo: String,
    pub predeterminada: Option<bool>,
    pub activo: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackModeloInput {
    pub codigo: String,
    pub config: Option<Value>,
    pub activo: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PackCapacidadInput {
    pub codigo: String,
    pub config: Option<Value>,
    pub activo: Option<bool>,
}

pub struct Packs {
    db: Arc<Db>,
}

impl Packs {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }
}

pub(super) fn agrupar(filas: Vec<sqlx::postgres::PgRow>) -> Result<Vec<Pack>, ApiError> {
    let mut packs: Vec<Pack> = Vec::new();
    let mut indices: HashMap<(String, i64), usize> = HashMap::new();
    for fila in filas {
        let codigo: String = fila.try_get("codigo").map_err(interno)?;
        let version: i32 = fila.try_get("version").map_err(interno)?;
        let clave = (codigo.clone(), i64::from(version));
        let indice = match indices.get(&clave) {
            Some(indice) => *indice,
            None => {
                let indice = packs.len();
                indices.insert(clave, indice);
                packs.push(Pack {
                    codigo,
                    version: i64::from(version),
                    vertical_codigo: String::new(),
                    nombre: String::new(),
                    descripcion: String::new(),
                    config: json!({}),
                    modelos: Vec::new(),
                    capacidades: Vec::new(),
                    regiones: Vec::new(),
                });
                indice
            }
        };
        let pack = &mut packs[indice];
        if pack.vertical_codigo.is_empty() {
            pack.vertical_codigo = fila.try_get("vertical_codigo").map_err(interno)?;
            pack.nombre = fila.try_get("nombre").map_err(interno)?;
            pack.descripcion = fila.try_get("descripcion").map_err(interno)?;
            pack.config = fila.try_get("config").map_err(interno)?;
            pack.modelos = modelos_de(fila.try_get("modelos").map_err(interno)?)?;
            pack.regiones = regiones_de(fila.try_get("regiones").map_err(interno)?);
        }
        let capacidad: Option<String> = fila.try_get("capacidad_codigo").map_err(interno)?;
        if let Some(capacidad_codigo) = capacidad {
            pack.capacidades.push(PackCapacidad {
                capacidad_codigo: capacidad_codigo.clone(),
                nombre: fila
                    .try_get::<Option<String>, _>("capacidad_nombre")
                    .map_err(interno)?
                    .unwrap_or_else(|| capacidad_codigo.clone()),
                modulo: fila
                    .try_get::<Option<String>, _>("capacidad_modulo")
                    .map_err(interno)?
                    .unwrap_or_else(|| "shared".to_string()),
                activo: fila
                    .try_get::<Option<bool>, _>("capacidad_activo")
                    .map_err(interno)?
                    .unwrap_or(false),
                config: fila
                    .try_get::<Option<Value>, _>("capacidad_config")
                    .map_err(interno)?
                    .unwrap_or_else(|| json!({})),
            });
        }
    }
    Ok(packs)
}

fn modelos_de(valor: Value) -> Result<Vec<PackModelo>, ApiError> {
    let Some(items) = valor.as_array() else {
        return Ok(Vec::new());
    };
    items
        .iter()
        .map(|item| {
            Ok(PackModelo {
                modelo_codigo: texto(item, "modelo_codigo"),
                nombre: texto(item, "nombre"),
                descripcion: texto(item, "descripcion"),
                activo: item.get("activo").and_then(Value::as_bool).unwrap_or(false),
                config: item.get("config").cloned().unwrap_or_else(|| json!({})),
            })
        })
        .collect()
}

fn regiones_de(valor: Value) -> Vec<RegionTributaria> {
    valor
        .as_array()
        .map(|items| items.iter().map(RegionTributaria::desde_valor).collect())
        .unwrap_or_default()
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::comunes::{es_codigo, es_codigo_pack};
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_validar_los_codigos_como_el_contrato() {
        assert!(es_codigo("pack_retail"));
        assert!(es_codigo("flota"));
        assert!(!es_codigo("Pack"));
        assert!(!es_codigo("pack retail"));
        assert!(es_codigo_pack("pack_belleza"));
        assert!(!es_codigo_pack("belleza"));
        assert!(!es_codigo_pack("pack_1belleza"));
        assert!(es_codigo_pack("pack_p1belleza"));
    }

    #[test]
    fn debe_agrupar_modelos_y_capacidades_por_pack() {
        let modelos = modelos_de(json!([
            {"modelo_codigo": "m1", "nombre": "Modelo", "descripcion": "", "activo": true, "config": {}}
        ]))
        .expect("modelos");
        assert_eq!(modelos.len(), 1);
        assert_eq!(modelos[0].modelo_codigo, "m1");
    }

    #[test]
    fn debe_mapear_el_config_ausente_a_un_objeto_vacio() {
        let modelos = modelos_de(json!([])).expect("modelos");
        assert!(modelos.is_empty());
    }
}
