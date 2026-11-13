//! Packs, verticales y modelos de operación.
//!
//! Réplica de `packs/packs.service.ts`: la lectura agrupa capacidades y
//! modelos por pack; la creación valida códigos y referencias activas.

use crate::db::Db;
use crate::errors::ApiError;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Arc;

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

    pub async fn listar(&self, vertical: Option<&str>) -> Result<Vec<Pack>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(SQL_LISTAR)
            .bind(vertical)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        agrupar(filas)
    }

    pub async fn obtener(&self, codigo: &str, version: i64) -> Result<Pack, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(SQL_OBTENER)
            .bind(codigo)
            .bind(version)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        agrupar(filas)?
            .into_iter()
            .next()
            .ok_or_else(|| ApiError::no_encontrado("Pack", &format!("{codigo}@{version}")))
    }

    pub async fn listar_modelos(&self) -> Result<Vec<ModeloOperacion>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, descripcion, config, activo
               FROM identity.modelos_operacion
              WHERE activo = true
              ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                Ok(ModeloOperacion {
                    codigo: fila.try_get("codigo").map_err(interno)?,
                    nombre: fila.try_get("nombre").map_err(interno)?,
                    descripcion: fila.try_get("descripcion").map_err(interno)?,
                    config: fila.try_get("config").map_err(interno)?,
                    activo: fila.try_get("activo").map_err(interno)?,
                })
            })
            .collect::<Result<_, _>>()
    }

    pub async fn listar_verticales(&self) -> Result<Vec<VerticalOperacion>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(
            "SELECT codigo, nombre, modelo_operacion, activo
               FROM identity.verticales
              WHERE activo = true
              ORDER BY codigo",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas
            .iter()
            .map(|fila| {
                Ok(VerticalOperacion {
                    codigo: fila.try_get("codigo").map_err(interno)?,
                    nombre: fila.try_get("nombre").map_err(interno)?,
                    modelo_operacion: fila.try_get("modelo_operacion").map_err(interno)?,
                    activo: fila.try_get("activo").map_err(interno)?,
                })
            })
            .collect::<Result<_, _>>()
    }

    pub async fn crear_modelo(
        &self,
        codigo: &str,
        nombre: &str,
        descripcion: Option<&str>,
        config: Option<Value>,
        activo: Option<bool>,
    ) -> Result<ModeloOperacion, ApiError> {
        let codigo = codigo_strict(codigo, "MODELO_OPERACION_INVALIDO")?;
        let nombre = nombre.trim();
        if !(2..=120).contains(&nombre.chars().count()) {
            return Err(ApiError::dominio(
                "MODELO_OPERACION_INVALIDO",
                "El nombre del modelo operativo no es válido.",
            ));
        }
        let config = config.unwrap_or_else(|| json!({}));
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "INSERT INTO identity.modelos_operacion (codigo, nombre, descripcion, config, activo)
             VALUES ($1, $2, $3, $4::jsonb, COALESCE($5, true))
             RETURNING codigo, nombre, descripcion, config, activo",
        )
        .bind(codigo)
        .bind(nombre)
        .bind(descripcion.unwrap_or("").trim())
        .bind(config)
        .bind(activo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear el modelo operativo."))?;
        tx.commit().await.map_err(interno)?;
        Ok(ModeloOperacion {
            codigo: fila.try_get("codigo").map_err(interno)?,
            nombre: fila.try_get("nombre").map_err(interno)?,
            descripcion: fila.try_get("descripcion").map_err(interno)?,
            config: fila.try_get("config").map_err(interno)?,
            activo: fila.try_get("activo").map_err(interno)?,
        })
    }

    pub async fn crear_vertical(
        &self,
        codigo: &str,
        nombre: &str,
        modelo_operacion: &str,
        activo: Option<bool>,
    ) -> Result<VerticalOperacion, ApiError> {
        let codigo = codigo_strict(codigo, "VERTICAL_INVALIDA")?;
        let nombre = nombre.trim();
        let modelo = modelo_operacion.trim();
        if !(2..=120).contains(&nombre.chars().count())
            || !(2..=160).contains(&modelo.chars().count())
        {
            return Err(ApiError::dominio(
                "VERTICAL_INVALIDA",
                "Los datos de la vertical no son válidos.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let modelo_valido = sqlx::query(
            "SELECT codigo FROM identity.modelos_operacion WHERE codigo = $1 AND activo = true",
        )
        .bind(modelo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if modelo_valido.is_none() {
            return Err(ApiError::no_encontrado("Modelo operativo", modelo));
        }
        let fila = sqlx::query(
            "INSERT INTO identity.verticales (codigo, nombre, modelo_operacion, activo)
             VALUES ($1, $2, $3, COALESCE($4, true))
             RETURNING codigo, nombre, modelo_operacion, activo",
        )
        .bind(codigo)
        .bind(nombre)
        .bind(modelo)
        .bind(activo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear la vertical."))?;
        tx.commit().await.map_err(interno)?;
        Ok(VerticalOperacion {
            codigo: fila.try_get("codigo").map_err(interno)?,
            nombre: fila.try_get("nombre").map_err(interno)?,
            modelo_operacion: fila.try_get("modelo_operacion").map_err(interno)?,
            activo: fila.try_get("activo").map_err(interno)?,
        })
    }

    pub async fn crear(&self, input: CrearPack) -> Result<Pack, ApiError> {
        let codigo = input.codigo.trim().to_lowercase();
        let vertical = codigo_strict(&input.vertical_codigo, "VERTICAL_INVALIDA")?;
        if !es_codigo_pack(&codigo) {
            return Err(ApiError::dominio(
                "PACK_INVALIDO",
                "El código del pack debe empezar por pack_.",
            ));
        }
        if input.version < 1 {
            return Err(ApiError::dominio(
                "PACK_INVALIDO",
                "La versión del pack debe ser un entero positivo.",
            ));
        }
        let nombre = input.nombre.trim();
        if !(2..=120).contains(&nombre.chars().count()) {
            return Err(ApiError::dominio(
                "PACK_INVALIDO",
                "El nombre del pack no es válido.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let vertical_activa =
            sqlx::query("SELECT 1 FROM identity.verticales WHERE codigo = $1 AND activo = true")
                .bind(&vertical)
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?;
        if vertical_activa.is_none() {
            return Err(ApiError::no_encontrado("Vertical", &vertical));
        }
        sqlx::query(
            "INSERT INTO identity.packs (codigo, version, vertical_codigo, nombre, descripcion, config)
             VALUES ($1, $2, $3, $4, $5, $6::jsonb)",
        )
        .bind(&codigo)
        .bind(input.version)
        .bind(&vertical)
        .bind(nombre)
        .bind(input.descripcion.as_deref().unwrap_or("").trim())
        .bind(input.config.unwrap_or_else(|| json!({})))
        .execute(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?;

        for modelo in input.modelos {
            let modelo_codigo = codigo_strict(&modelo.codigo, "MODELO_OPERACION_INVALIDO")?;
            let insertado = sqlx::query(
                "INSERT INTO identity.pack_modelos
                   (pack_codigo, pack_version, modelo_codigo, config, activo)
                 SELECT $1, $2, codigo, $4::jsonb, COALESCE($5, true)
                   FROM identity.modelos_operacion
                  WHERE codigo = $3 AND activo = true",
            )
            .bind(&codigo)
            .bind(input.version)
            .bind(&modelo_codigo)
            .bind(modelo.config.unwrap_or_else(|| json!({})))
            .bind(modelo.activo)
            .execute(&mut *tx)
            .await
            .map_err(conflicto_o_interno)?;
            if insertado.rows_affected() == 0 {
                return Err(ApiError::no_encontrado("Modelo operativo", &modelo_codigo));
            }
        }

        for capacidad in input.capacidades {
            let capacidad_codigo = codigo_strict(&capacidad.codigo, "CAPACIDAD_INVALIDA")?;
            let insertada = sqlx::query(
                "INSERT INTO identity.pack_capacidades
                   (pack_codigo, pack_version, capacidad_codigo, config, activo)
                 SELECT $1, $2, codigo, $4::jsonb, COALESCE($5, true)
                   FROM identity.capacidades
                  WHERE codigo = $3 AND activo = true",
            )
            .bind(&codigo)
            .bind(input.version)
            .bind(&capacidad_codigo)
            .bind(capacidad.config.unwrap_or_else(|| json!({})))
            .bind(capacidad.activo)
            .execute(&mut *tx)
            .await
            .map_err(conflicto_o_interno)?;
            if insertada.rows_affected() == 0 {
                return Err(ApiError::no_encontrado("Capacidad", &capacidad_codigo));
            }
        }

        sqlx::query(
            "INSERT INTO identity.vertical_capacidades AS vc
               (vertical_codigo, capacidad_codigo, config, activo)
             SELECT $1, pc.capacidad_codigo, pc.config, pc.activo
               FROM identity.pack_capacidades pc
              WHERE pc.pack_codigo = $2 AND pc.pack_version = $3
             ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING",
        )
        .bind(&vertical)
        .bind(&codigo)
        .bind(input.version)
        .execute(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?;
        tx.commit().await.map_err(interno)?;
        self.obtener(&codigo, input.version).await
    }
}

const SQL_LISTAR: &str =
    "SELECT p.codigo, p.version, p.vertical_codigo, p.nombre, p.descripcion, p.config,
        pc.capacidad_codigo, c.nombre AS capacidad_nombre, c.modulo AS capacidad_modulo,
        pc.activo AS capacidad_activo, pc.config AS capacidad_config,
        COALESCE(modelos.items, '[]'::jsonb) AS modelos
   FROM identity.packs p
   LEFT JOIN identity.pack_capacidades pc
     ON pc.pack_codigo = p.codigo AND pc.pack_version = p.version
   LEFT JOIN identity.capacidades c ON c.codigo = pc.capacidad_codigo
   LEFT JOIN LATERAL (
     SELECT jsonb_agg(jsonb_build_object(
       'modelo_codigo', pm.modelo_codigo,
       'nombre', mo.nombre,
       'descripcion', mo.descripcion,
       'activo', pm.activo,
       'config', pm.config
     ) ORDER BY pm.modelo_codigo) AS items
       FROM identity.pack_modelos pm
       JOIN identity.modelos_operacion mo ON mo.codigo = pm.modelo_codigo
      WHERE pm.pack_codigo = p.codigo AND pm.pack_version = p.version
   ) modelos ON true
  WHERE p.activo = true
    AND ($1::text IS NULL OR p.vertical_codigo = $1)
  ORDER BY p.vertical_codigo, p.codigo, pc.capacidad_codigo";

const SQL_OBTENER: &str =
    "SELECT p.codigo, p.version, p.vertical_codigo, p.nombre, p.descripcion, p.config,
        pc.capacidad_codigo, c.nombre AS capacidad_nombre, c.modulo AS capacidad_modulo,
        pc.activo AS capacidad_activo, pc.config AS capacidad_config,
        COALESCE(modelos.items, '[]'::jsonb) AS modelos
   FROM identity.packs p
   LEFT JOIN identity.pack_capacidades pc
     ON pc.pack_codigo = p.codigo AND pc.pack_version = p.version
   LEFT JOIN identity.capacidades c ON c.codigo = pc.capacidad_codigo
   LEFT JOIN LATERAL (
     SELECT jsonb_agg(jsonb_build_object(
       'modelo_codigo', pm.modelo_codigo,
       'nombre', mo.nombre,
       'descripcion', mo.descripcion,
       'activo', pm.activo,
       'config', pm.config
     ) ORDER BY pm.modelo_codigo) AS items
       FROM identity.pack_modelos pm
       JOIN identity.modelos_operacion mo ON mo.codigo = pm.modelo_codigo
      WHERE pm.pack_codigo = p.codigo AND pm.pack_version = p.version
   ) modelos ON true
  WHERE p.codigo = $1 AND p.version = $2 AND p.activo = true
  ORDER BY pc.capacidad_codigo";

fn agrupar(filas: Vec<sqlx::postgres::PgRow>) -> Result<Vec<Pack>, ApiError> {
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

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn codigo_strict(valor: &str, error: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_lowercase();
    if !es_codigo(&codigo) {
        return Err(ApiError::dominio(
            error,
            "El código contiene caracteres no permitidos.",
        ));
    }
    Ok(codigo)
}

fn es_codigo(valor: &str) -> bool {
    let mut chars = valor.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&valor.len())
        && valor
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn es_codigo_pack(valor: &str) -> bool {
    let Some(resto) = valor.strip_prefix("pack_") else {
        return false;
    };
    let mut chars = resto.chars();
    matches!(chars.next(), Some(primero) if primero.is_ascii_lowercase())
        && (2..=40).contains(&resto.len())
        && resto
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn conflicto_o_interno(error: sqlx::Error) -> ApiError {
    if matches!(&error, sqlx::Error::Database(db) if db.code().as_deref() == Some("23505")) {
        return ApiError::conflicto("Ya existe un registro con ese código.");
    }
    interno(error)
}

fn interno(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "identity.db",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

#[cfg(test)]
mod tests {
    use super::*;

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
