//! Packs, verticales y modelos de operación.
//!
//! Réplica de `packs/packs.service.ts`: la lectura agrupa capacidades y
//! modelos por pack; la creación valida códigos y referencias activas.

use crate::db::Db;
use crate::dominio::regiones::{region_de_fila, RegionTributaria};
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

    pub async fn regiones(
        &self,
        codigo: &str,
        version: i32,
    ) -> Result<Vec<RegionTributaria>, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let existe = sqlx::query(
            "SELECT 1 FROM identity.packs WHERE codigo = $1 AND version = $2 AND activo",
        )
        .bind(codigo)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if existe.is_none() {
            return Err(ApiError::no_encontrado(
                "Pack",
                &format!("{codigo}@{version}"),
            ));
        }
        let filas = sqlx::query(
            "SELECT j.codigo_pais, j.nombre, j.pais_iso, j.subdivision, j.moneda,
                    j.moneda_iso, j.simbolo_moneda, j.locale, j.zona_horaria,
                    j.tasa_iva, j.tasa_ir, j.periodicidad_declaracion,
                    j.documento_fiscal, j.perfil, j.leyes, j.vigente_desde, j.activa,
                    pr.predeterminada
               FROM identity.pack_regiones pr
               JOIN finance.jurisdicciones j ON j.codigo_pais = pr.region_codigo
              WHERE pr.pack_codigo = $1 AND pr.pack_version = $2
              ORDER BY pr.predeterminada DESC, j.codigo_pais",
        )
        .bind(codigo)
        .bind(version)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas.iter().map(region_de_fila).collect()
    }

    pub async fn reemplazar_regiones(
        &self,
        codigo: &str,
        version: i32,
        mut regiones: Vec<PackRegionInput>,
    ) -> Result<Pack, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let existe = sqlx::query(
            "SELECT 1 FROM identity.packs WHERE codigo = $1 AND version = $2 AND activo",
        )
        .bind(codigo)
        .bind(version)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if existe.is_none() {
            return Err(ApiError::no_encontrado(
                "Pack",
                &format!("{codigo}@{version}"),
            ));
        }
        if regiones.is_empty() {
            return Err(ApiError::dominio(
                "REGION_NO_DISPONIBLE",
                "El pack debe declarar al menos una región.",
            ));
        }
        if regiones
            .iter()
            .filter(|region| region.predeterminada == Some(true))
            .count()
            > 1
        {
            return Err(ApiError::dominio(
                "REGION_PREDETERMINADA_UNICA",
                "Un pack solo puede tener una región predeterminada.",
            ));
        }
        let mut predeterminada_asignada = regiones
            .iter()
            .any(|region| region.predeterminada == Some(true));
        for (indice, region) in regiones.iter_mut().enumerate() {
            region.codigo = region.codigo.trim().to_uppercase();
            let existe = sqlx::query(
                "SELECT 1 FROM finance.jurisdicciones
                  WHERE codigo_pais = $1 AND activa = true",
            )
            .bind(&region.codigo)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
            if existe.is_none() {
                return Err(ApiError::no_encontrado("Region tributaria", &region.codigo));
            }
            if !predeterminada_asignada && indice == 0 {
                region.predeterminada = Some(true);
                predeterminada_asignada = true;
            }
        }
        let usadas = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT region_tributaria
               FROM identity.empresas
              WHERE pack_codigo = $1 AND pack_version = $2
                AND region_tributaria IS NOT NULL",
        )
        .bind(codigo)
        .bind(version)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        let en_uso: Vec<String> = usadas
            .into_iter()
            .filter(|region| {
                !regiones
                    .iter()
                    .any(|nueva| nueva.codigo == *region && nueva.activo.unwrap_or(true))
            })
            .collect();
        if !en_uso.is_empty() {
            return Err(ApiError::dominio(
                "REGION_EN_USO",
                "No se puede retirar una región que ya usan empresas de este pack.",
            )
            .con_detalles(json!({ "regiones": en_uso })));
        }
        sqlx::query(
            "DELETE FROM identity.pack_regiones
              WHERE pack_codigo = $1 AND pack_version = $2",
        )
        .bind(codigo)
        .bind(version)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        for region in &regiones {
            sqlx::query(
                "INSERT INTO identity.pack_regiones
                   (pack_codigo, pack_version, region_codigo, predeterminada, activo)
                 VALUES ($1, $2, $3, $4, COALESCE($5, true))",
            )
            .bind(codigo)
            .bind(version)
            .bind(&region.codigo)
            .bind(region.predeterminada.unwrap_or(false))
            .bind(region.activo)
            .execute(&mut *tx)
            .await
            .map_err(conflicto_o_interno)?;
        }
        tx.commit().await.map_err(interno)?;
        self.obtener(codigo, i64::from(version)).await
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

        // Regiones tributarias del pack. Si el cliente no las declara, se
        // asignan las regiones activas de la plataforma (compatibilidad y
        // arranque con un solo país). El admin puede restringirlas después.
        let mut regiones = input.regiones;
        if regiones.is_empty() {
            // Una versión nueva hereda las regiones de la versión anterior.
            // Si es la primera versión, se usan las regiones activas.
            let anteriores = sqlx::query(
                "SELECT region_codigo, predeterminada, activo
                   FROM identity.pack_regiones
                  WHERE pack_codigo = $1 AND pack_version < $2 AND activo = true
                  ORDER BY predeterminada DESC, region_codigo",
            )
            .bind(&codigo)
            .bind(input.version)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
            if anteriores.is_empty() {
                let filas = sqlx::query(
                    "SELECT codigo_pais FROM finance.jurisdicciones
                      WHERE activa = true ORDER BY codigo_pais",
                )
                .fetch_all(&mut *tx)
                .await
                .map_err(interno)?;
                regiones = filas
                    .iter()
                    .map(|fila| {
                        Ok(PackRegionInput {
                            codigo: fila.try_get("codigo_pais").map_err(interno)?,
                            predeterminada: None,
                            activo: None,
                        })
                    })
                    .collect::<Result<Vec<_>, ApiError>>()?;
            } else {
                regiones = anteriores
                    .iter()
                    .map(|fila| {
                        Ok(PackRegionInput {
                            codigo: fila.try_get("region_codigo").map_err(interno)?,
                            predeterminada: Some(fila.try_get("predeterminada").map_err(interno)?),
                            activo: Some(fila.try_get("activo").map_err(interno)?),
                        })
                    })
                    .collect::<Result<Vec<_>, ApiError>>()?;
            }
        }
        if regiones.is_empty() {
            return Err(ApiError::dominio(
                "REGION_NO_DISPONIBLE",
                "No hay regiones tributarias activas para asignar al pack.",
            ));
        }
        if regiones
            .iter()
            .filter(|region| region.predeterminada == Some(true))
            .count()
            > 1
        {
            return Err(ApiError::dominio(
                "REGION_PREDETERMINADA_UNICA",
                "Un pack solo puede tener una región predeterminada.",
            ));
        }
        let mut predeterminada_asignada = regiones
            .iter()
            .any(|region| region.predeterminada == Some(true));
        for (indice, region) in regiones.iter().enumerate() {
            let region_codigo = region.codigo.trim().to_uppercase();
            let existe = sqlx::query(
                "SELECT 1 FROM finance.jurisdicciones
                  WHERE codigo_pais = $1 AND activa = true",
            )
            .bind(&region_codigo)
            .fetch_optional(&mut *tx)
            .await
            .map_err(interno)?;
            if existe.is_none() {
                return Err(ApiError::no_encontrado("Region tributaria", &region_codigo));
            }
            let es_predeterminada =
                region.predeterminada == Some(true) || (!predeterminada_asignada && indice == 0);
            predeterminada_asignada |= es_predeterminada;
            sqlx::query(
                "INSERT INTO identity.pack_regiones
                   (pack_codigo, pack_version, region_codigo, predeterminada, activo)
                 VALUES ($1, $2, $3, $4, COALESCE($5, true))",
            )
            .bind(&codigo)
            .bind(input.version)
            .bind(&region_codigo)
            .bind(es_predeterminada)
            .bind(region.activo)
            .execute(&mut *tx)
            .await
            .map_err(conflicto_o_interno)?;
        }

        tx.commit().await.map_err(interno)?;
        self.obtener(&codigo, input.version).await
    }
}

const SQL_LISTAR: &str =
    "SELECT p.codigo, p.version, p.vertical_codigo, p.nombre, p.descripcion, p.config,
        pc.capacidad_codigo, c.nombre AS capacidad_nombre, c.modulo AS capacidad_modulo,
        pc.activo AS capacidad_activo, pc.config AS capacidad_config,
        COALESCE(modelos.items, '[]'::jsonb) AS modelos,
        COALESCE(regiones.items, '[]'::jsonb) AS regiones
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
   LEFT JOIN LATERAL (
     SELECT jsonb_agg(jsonb_build_object(
       'codigo', j.codigo_pais,
       'nombre', j.nombre,
       'pais_iso', j.pais_iso,
       'subdivision', j.subdivision,
       'moneda', j.moneda,
       'moneda_iso', j.moneda_iso,
       'simbolo_moneda', j.simbolo_moneda,
       'locale', j.locale,
       'zona_horaria', j.zona_horaria,
       'tasa_iva', j.tasa_iva,
       'tasa_ir', j.tasa_ir,
       'periodicidad_declaracion', j.periodicidad_declaracion,
       'documento_fiscal', j.documento_fiscal,
       'perfil', j.perfil,
       'leyes', j.leyes,
       'vigente_desde', j.vigente_desde,
       'predeterminada', pr.predeterminada
     ) ORDER BY pr.predeterminada DESC, j.codigo_pais) AS items
       FROM identity.pack_regiones pr
       JOIN finance.jurisdicciones j ON j.codigo_pais = pr.region_codigo
      WHERE pr.pack_codigo = p.codigo AND pr.pack_version = p.version
        AND pr.activo AND j.activa
   ) regiones ON true
  WHERE p.activo = true
    AND ($1::text IS NULL OR p.vertical_codigo = $1)
  ORDER BY p.vertical_codigo, p.codigo, pc.capacidad_codigo";

const SQL_OBTENER: &str =
    "SELECT p.codigo, p.version, p.vertical_codigo, p.nombre, p.descripcion, p.config,
        pc.capacidad_codigo, c.nombre AS capacidad_nombre, c.modulo AS capacidad_modulo,
        pc.activo AS capacidad_activo, pc.config AS capacidad_config,
        COALESCE(modelos.items, '[]'::jsonb) AS modelos,
        COALESCE(regiones.items, '[]'::jsonb) AS regiones
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
   LEFT JOIN LATERAL (
     SELECT jsonb_agg(jsonb_build_object(
       'codigo', j.codigo_pais,
       'nombre', j.nombre,
       'pais_iso', j.pais_iso,
       'subdivision', j.subdivision,
       'moneda', j.moneda,
       'moneda_iso', j.moneda_iso,
       'simbolo_moneda', j.simbolo_moneda,
       'locale', j.locale,
       'zona_horaria', j.zona_horaria,
       'tasa_iva', j.tasa_iva,
       'tasa_ir', j.tasa_ir,
       'periodicidad_declaracion', j.periodicidad_declaracion,
       'documento_fiscal', j.documento_fiscal,
       'perfil', j.perfil,
       'leyes', j.leyes,
       'vigente_desde', j.vigente_desde,
       'predeterminada', pr.predeterminada
     ) ORDER BY pr.predeterminada DESC, j.codigo_pais) AS items
       FROM identity.pack_regiones pr
       JOIN finance.jurisdicciones j ON j.codigo_pais = pr.region_codigo
      WHERE pr.pack_codigo = p.codigo AND pr.pack_version = p.version
        AND pr.activo AND j.activa
   ) regiones ON true
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
