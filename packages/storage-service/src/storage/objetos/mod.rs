//! Objetos, cuotas y URLs prefirmadas (ADR-24/ADR-28).

use crate::almacen::{AlmacenObjetos, ObjetoLeido};
use crate::auth::UsuarioContexto;
use crate::config::Config;
use crate::db::{error_db, jsonb, timestamp, timestamp_opcional, uuid_opcional, uuid_texto, Db};
use crate::errors::ApiError;
use crate::logs;
use crate::outbox::{Outbox, STORAGE_OBJETO_ELIMINADO, STORAGE_OBJETO_SUBIDO};
use crate::storage::buckets::{error_almacen, Buckets};
use crate::storage::pagination::{crear_pagina, parsear_paginacion, Pagina};
use base64::Engine;
use chrono::{Duration, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sqlx::postgres::PgRow;
use sqlx::{Postgres, QueryBuilder, Row, Transaction};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ObjetoStorage {
    pub id: String,
    pub tenant_id: String,
    pub bucket: String,
    pub objeto: String,
    pub nombre: String,
    pub content_type: String,
    pub carpeta: String,
    pub descripcion: String,
    pub etiquetas: BTreeMap<String, String>,
    pub tamano_bytes: i64,
    pub etag: Option<String>,
    pub estado: String,
    pub publico: bool,
    pub subido_por: Option<String>,
    pub creado_en: String,
    pub confirmado_en: Option<String>,
    pub eliminado_en: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SolicitudSubida {
    pub objeto_id: String,
    pub bucket: String,
    pub objeto: String,
    pub url_subida: String,
    pub metodo: String,
    pub headers: BTreeMap<String, String>,
    pub expira_en: String,
    pub max_bytes: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UrlDescarga {
    pub url_descarga: String,
    pub expira_en: String,
    pub nombre: String,
    pub content_type: String,
    pub tamano_bytes: i64,
}

pub struct ObjetoPublico {
    pub cuerpo: ObjetoLeido,
    pub content_type: String,
    pub tamano_bytes: i64,
    pub etag: Option<String>,
    pub nombre: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjetoPublicoMeta {
    pub content_type: String,
    pub tamano_bytes: i64,
    pub etag: Option<String>,
    pub nombre: String,
}

#[derive(Debug, Clone, Default)]
pub struct DatosSubida {
    pub nombre: String,
    pub content_type: String,
    pub tamano_bytes: i64,
    pub carpeta: Option<String>,
    pub descripcion: Option<String>,
    pub etiquetas: Option<BTreeMap<String, String>>,
    pub publico: bool,
}

pub struct Objetos {
    db: Arc<Db>,
    buckets: Arc<Buckets>,
    outbox: Arc<Outbox>,
    almacen: Arc<dyn AlmacenObjetos>,
    config: Arc<Config>,
}

mod comunes;
mod consultas;
mod mantenimiento;
mod subidas;

impl Objetos {
    pub fn nuevo(
        db: Arc<Db>,
        buckets: Arc<Buckets>,
        outbox: Arc<Outbox>,
        almacen: Arc<dyn AlmacenObjetos>,
        config: Arc<Config>,
    ) -> Self {
        Self {
            db,
            buckets,
            outbox,
            almacen,
            config,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FiltrosObjetos {
    pub q: Option<String>,
    pub carpeta: Option<String>,
    pub estado: Option<String>,
    pub pagina: Option<i64>,
    pub limite: Option<i64>,
}

const COLUMNAS: &str = "id, tenant_id, bucket, objeto, nombre_original, content_type, carpeta,
  descripcion, etiquetas, tamano_bytes, etag, estado, publico, subido_por, creado_en,
  confirmado_en, eliminado_en";

fn aplicar_filtros(builder: &mut QueryBuilder<'_, Postgres>, filtros: &FiltrosObjetos) {
    if let Some(estado) = &filtros.estado {
        builder.push(" AND estado = ");
        builder.push_bind(estado.clone());
    } else {
        builder.push(" AND estado <> 'eliminado'");
    }
    if let Some(carpeta) = &filtros.carpeta {
        builder.push(" AND carpeta = ");
        builder.push_bind(carpeta.to_lowercase());
    }
    if let Some(q) = &filtros.q {
        builder.push(" AND LOWER(nombre_original) LIKE ");
        builder.push_bind(format!("%{}%", q.to_lowercase()));
    }
}

fn serializar(fila: &PgRow) -> Result<ObjetoStorage, ApiError> {
    let etiquetas = jsonb(fila, "etiquetas").map_err(error_db)?;
    let etiquetas: BTreeMap<String, String> = serde_json::from_value(etiquetas).unwrap_or_default();
    Ok(ObjetoStorage {
        id: uuid_texto(fila, "id").map_err(error_db)?,
        tenant_id: uuid_texto(fila, "tenant_id").map_err(error_db)?,
        bucket: fila.try_get::<String, _>("bucket").map_err(error_db)?,
        objeto: fila.try_get::<String, _>("objeto").map_err(error_db)?,
        nombre: fila
            .try_get::<String, _>("nombre_original")
            .map_err(error_db)?,
        content_type: fila
            .try_get::<String, _>("content_type")
            .map_err(error_db)?,
        carpeta: fila.try_get::<String, _>("carpeta").map_err(error_db)?,
        descripcion: fila.try_get::<String, _>("descripcion").map_err(error_db)?,
        etiquetas,
        tamano_bytes: fila.try_get::<i64, _>("tamano_bytes").map_err(error_db)?,
        etag: fila
            .try_get::<Option<String>, _>("etag")
            .map_err(error_db)?,
        estado: fila.try_get::<String, _>("estado").map_err(error_db)?,
        publico: fila.try_get::<bool, _>("publico").map_err(error_db)?,
        subido_por: uuid_opcional(fila, "subido_por").map_err(error_db)?,
        creado_en: timestamp(fila, "creado_en").map_err(error_db)?,
        confirmado_en: timestamp_opcional(fila, "confirmado_en").map_err(error_db)?,
        eliminado_en: timestamp_opcional(fila, "eliminado_en").map_err(error_db)?,
    })
}

fn es_content_type_publico(content_type: &str) -> bool {
    let tipo = content_type.trim().to_lowercase();
    if tipo == "image/svg+xml" {
        return false;
    }
    tipo.starts_with("image/") || tipo == "video/mp4" || tipo == "video/webm"
}

fn sanear_nombre(valor: &str) -> String {
    let base = valor.split(['/', '\\']).next_back().unwrap_or("");
    let limpio = base
        .chars()
        .map(|caracter| {
            if caracter.is_ascii_alphanumeric() || matches!(caracter, '.' | '_' | '-') {
                caracter
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_start_matches(['.', '_'])
        .chars()
        .take(180)
        .collect::<String>();
    if limpio.is_empty() {
        "archivo".to_string()
    } else {
        limpio
    }
}

fn sanear_carpeta(valor: Option<&str>) -> String {
    let limpio = valor
        .unwrap_or("")
        .trim()
        .to_lowercase()
        .chars()
        .map(|caracter| {
            if caracter.is_ascii_alphanumeric() || matches!(caracter, '.' | '_' | '-') {
                caracter
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['-', '.'])
        .chars()
        .take(40)
        .collect::<String>();
    if limpio.is_empty() {
        "general".to_string()
    } else {
        limpio
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_sanear_nombre_y_carpeta() {
        assert_eq!(sanear_nombre("../../foto final.JPG"), "foto_final.JPG");
        assert_eq!(sanear_nombre(""), "archivo");
        assert_eq!(sanear_carpeta(Some("../Productos 2026")), "productos-2026");
        assert_eq!(sanear_carpeta(None), "general");
    }

    #[test]
    fn debe_limitar_los_content_types_publicos() {
        assert!(es_content_type_publico("image/png"));
        assert!(es_content_type_publico("video/webm"));
        assert!(!es_content_type_publico("image/svg+xml"));
        assert!(!es_content_type_publico("application/pdf"));
    }
}
