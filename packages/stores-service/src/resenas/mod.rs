//! Reseñas de comprador por item, con moderación.

use crate::clients::Catalogo;
use crate::db::Db;
use crate::errors::ApiError;
use crate::ofertas::{crear_pagina, parsear_paginacion};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

const COLUMNAS: &str = "id, empresa_id, item_id, comprador_id, calificacion,
  titulo, comentario, estado, creado_en, actualizado_en";

pub struct Resenas {
    db: Arc<Db>,
    catalogo: Arc<dyn Catalogo>,
}

impl Resenas {
    pub fn nueva(db: Arc<Db>, catalogo: Arc<dyn Catalogo>) -> Self {
        Self { db, catalogo }
    }

    pub async fn listar_publicas(
        &self,
        item_id: &str,
        pagina: Option<&str>,
        limite: Option<&str>,
    ) -> Result<Value, ApiError> {
        if Uuid::parse_str(item_id).is_err() {
            return Err(ApiError::no_encontrado("Item", item_id));
        }
        let paginacion = parsear_paginacion(pagina, limite);
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM stores.resenas
              WHERE item_id = $1::uuid AND estado = 'publicada'",
        )
        .bind(item_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(crate::db::interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM stores.resenas
              WHERE item_id = $1::uuid AND estado = 'publicada'
              ORDER BY creado_en DESC LIMIT $2 OFFSET $3"
        ))
        .bind(item_id)
        .bind(paginacion.limite)
        .bind((paginacion.pagina - 1) * paginacion.limite)
        .fetch_all(self.db.pool())
        .await
        .map_err(crate::db::interno)?;
        let items = filas
            .iter()
            .map(fila_a_json)
            .collect::<Result<Vec<_>, _>>()?;
        let promedio: Option<f64> = sqlx::query_scalar(
            "SELECT AVG(calificacion)::float8 FROM stores.resenas
              WHERE item_id = $1::uuid AND estado = 'publicada'",
        )
        .bind(item_id)
        .fetch_one(self.db.pool())
        .await
        .map_err(crate::db::interno)?;
        let mut pagina_json = crear_pagina(items, total, paginacion);
        if let Some(objeto) = pagina_json.as_object_mut() {
            objeto.insert(
                "promedio".into(),
                json!(promedio
                    .map(|valor| (valor * 100.0).round() / 100.0)
                    .unwrap_or(0.0)),
            );
        }
        Ok(pagina_json)
    }

    pub async fn listar_comprador(&self, comprador_id: &str) -> Result<Vec<Value>, ApiError> {
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM stores.resenas
              WHERE comprador_id = $1::uuid ORDER BY creado_en DESC LIMIT 100"
        ))
        .bind(comprador_id)
        .fetch_all(self.db.pool())
        .await
        .map_err(crate::db::interno)?;
        filas.iter().map(fila_a_json).collect()
    }

    pub async fn crear(&self, comprador_id: &str, datos: &Value) -> Result<Value, ApiError> {
        let item_id = texto(datos, "item_id");
        let item = self
            .catalogo
            .item(&item_id)
            .await
            .ok_or_else(|| ApiError::no_encontrado("Item", &item_id))?;
        let (calificacion, titulo, comentario) = validar(datos)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO stores.resenas
               (empresa_id, item_id, comprador_id, calificacion, titulo, comentario)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6)
             RETURNING {COLUMNAS}"
        ))
        .bind(&item.empresa_id)
        .bind(&item_id)
        .bind(comprador_id)
        .bind(calificacion)
        .bind(&titulo)
        .bind(&comentario)
        .fetch_optional(self.db.pool())
        .await
        .map_err(conflicto_o_interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear la reseña."))?;
        fila_a_json(&fila)
    }

    pub async fn actualizar(
        &self,
        id: &str,
        comprador_id: &str,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.obtener(id, Some(comprador_id)).await?;
        let (calificacion, titulo, comentario) = validar_combinado(&actual, datos)?;
        let fila = sqlx::query(&format!(
            "UPDATE stores.resenas
                SET calificacion = $2, titulo = $3, comentario = $4,
                    estado = 'pendiente', actualizado_en = NOW()
              WHERE id = $1::uuid AND comprador_id = $5::uuid
              RETURNING {COLUMNAS}"
        ))
        .bind(id)
        .bind(calificacion)
        .bind(&titulo)
        .bind(&comentario)
        .bind(comprador_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(crate::db::interno)?
        .ok_or_else(|| ApiError::no_encontrado("Resena", id))?;
        fila_a_json(&fila)
    }

    pub async fn eliminar(&self, id: &str, comprador_id: &str) -> Result<(), ApiError> {
        let fila = sqlx::query(
            "DELETE FROM stores.resenas
              WHERE id = $1::uuid AND (comprador_id = $2::uuid OR $3::boolean)
              RETURNING id",
        )
        .bind(id)
        .bind(comprador_id)
        .bind(false)
        .fetch_optional(self.db.pool())
        .await
        .map_err(crate::db::interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Resena", id));
        }
        Ok(())
    }

    pub async fn moderar(&self, id: &str, estado: &str) -> Result<Value, ApiError> {
        if !["publicada", "rechazada", "pendiente"].contains(&estado) {
            return Err(ApiError::dominio(
                "ESTADO_RESENA_INVALIDO",
                "El estado de la reseña no es válido.",
            ));
        }
        let fila = sqlx::query(&format!(
            "UPDATE stores.resenas SET estado = $2, actualizado_en = NOW()
              WHERE id = $1::uuid RETURNING {COLUMNAS}"
        ))
        .bind(id)
        .bind(estado)
        .fetch_optional(self.db.pool())
        .await
        .map_err(crate::db::interno)?
        .ok_or_else(|| ApiError::no_encontrado("Resena", id))?;
        fila_a_json(&fila)
    }

    async fn obtener(&self, id: &str, comprador_id: Option<&str>) -> Result<Value, ApiError> {
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM stores.resenas
              WHERE id = $1::uuid AND ($2::uuid IS NULL OR comprador_id = $2::uuid)"
        ))
        .bind(id)
        .bind(comprador_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(crate::db::interno)?
        .ok_or_else(|| ApiError::no_encontrado("Resena", id))?;
        fila_a_json(&fila)
    }
}

fn validar(datos: &Value) -> Result<(i32, String, String), ApiError> {
    let calificacion = datos
        .get("calificacion")
        .and_then(Value::as_i64)
        .filter(|valor| (1..=5).contains(valor))
        .ok_or_else(|| {
            ApiError::validacion(json!([{
                "campo": "calificacion",
                "restricciones": ["min:1", "max:5"],
            }]))
        })? as i32;
    let titulo = texto(datos, "titulo");
    let comentario = texto(datos, "comentario");
    if titulo.chars().count() > 120 || comentario.chars().count() > 2000 {
        return Err(ApiError::validacion(json!([{
            "campo": "titulo/comentario",
            "restricciones": ["maxLength"],
        }])));
    }
    Ok((calificacion, titulo, comentario))
}

fn validar_combinado(actual: &Value, datos: &Value) -> Result<(i32, String, String), ApiError> {
    let mut combinado = actual.clone();
    if let Some(objeto) = combinado.as_object_mut() {
        for (clave, valor) in datos.as_object().into_iter().flatten() {
            objeto.insert(clave.clone(), valor.clone());
        }
    }
    validar(&combinado)
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn fila_a_json(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    crate::db::fila_a_json(fila).map_err(crate::db::interno)
}

fn conflicto_o_interno(error: sqlx::Error) -> ApiError {
    if let Some(codigo) = error.as_database_error().and_then(|db| db.code()) {
        if codigo == "23505" {
            return ApiError::conflicto("Ya existe una reseña de este comprador para el item.");
        }
    }
    crate::db::interno(error)
}
