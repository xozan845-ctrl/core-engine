//! Promociones por empresa (2x1, descuento por cantidad/porcentaje y regalo).
//!
//! Portado de `catalog-service` (ADR-0018). La promoción referencia un item por
//! `item_id`/`sku`, pero no posee stock ni precios base: solo su vigencia y
//! regla comercial.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::sync::Arc;

const TIPOS: [&str; 4] = [
    "2x1",
    "descuento_cantidad",
    "descuento_porcentaje",
    "regalo",
];
const COLUMNAS: &str = "id, empresa_id, nombre, tipo, item_id, sku,
  cantidad_requerida, cantidad_gratis, descuento_porcentaje,
  fecha_desde::text AS fecha_desde, fecha_hasta::text AS fecha_hasta,
  estado, creado_en, actualizado_en";
const RETURNING: &str = "RETURNING id, empresa_id, nombre, tipo, item_id, sku,
  cantidad_requerida, cantidad_gratis, descuento_porcentaje,
  fecha_desde::text AS fecha_desde, fecha_hasta::text AS fecha_hasta,
  estado, creado_en, actualizado_en";

#[derive(Debug, Clone, PartialEq)]
struct Normalizada {
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

pub struct Promociones {
    db: Arc<Db>,
}

impl Promociones {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(&self, empresa_id: &str, filtros: &Value) -> Result<Value, ApiError> {
        let paginacion = parsear_paginacion(
            filtro_texto(filtros, "pagina").as_deref(),
            filtro_texto(filtros, "limite").as_deref(),
        );
        let mut params: Vec<String> = vec![empresa_id.to_string()];
        let mut condiciones = vec![
            "empresa_id = $1::uuid".to_string(),
            "eliminado_en IS NULL".to_string(),
        ];
        if let Some(estado) = filtro_texto(filtros, "estado").filter(|valor| !valor.is_empty()) {
            params.push(estado);
            condiciones.push(format!("estado = ${}", params.len()));
        }
        let where_clause = format!("WHERE {}", condiciones.join(" AND "));

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total =
            format!("SELECT COUNT(*)::int AS n FROM pricing.promociones {where_clause}");
        let mut consulta_total = sqlx::query(&sql_total);
        for parametro in &params {
            consulta_total = consulta_total.bind(parametro);
        }
        let total: i32 = consulta_total
            .fetch_one(&mut *tx)
            .await
            .map_err(interno)?
            .get("n");
        let sql = format!(
            "SELECT {COLUMNAS}
               FROM pricing.promociones {where_clause}
              ORDER BY creado_en DESC LIMIT ${} OFFSET ${}",
            params.len() + 1,
            params.len() + 2
        );
        let mut consulta = sqlx::query(&sql);
        for parametro in &params {
            consulta = consulta.bind(parametro);
        }
        let filas = consulta
            .bind(paginacion.limite)
            .bind((paginacion.pagina - 1) * paginacion.limite)
            .fetch_all(&mut *tx)
            .await
            .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let promociones = filas
            .iter()
            .map(|fila| {
                fila_a_json(fila)
                    .map_err(interno)
                    .map(|valor| serializar(&valor))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crear_pagina(promociones, total as i64, paginacion))
    }

    pub async fn obtener(&self, id: &str, empresa_id: &str) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS}
               FROM pricing.promociones
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL"
        ))
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        fila.map(|fila| fila_a_json(&fila).map(|valor| serializar(&valor)))
            .transpose()
            .map_err(interno)
    }

    pub async fn crear(&self, empresa_id: &str, datos: &Value) -> Result<Value, ApiError> {
        let normalizada = self.normalizar(empresa_id, datos).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO pricing.promociones
               (empresa_id, nombre, tipo, item_id, sku, cantidad_requerida, cantidad_gratis,
                descuento_porcentaje, fecha_desde, fecha_hasta, estado)
             VALUES ($1::uuid, $2, $3, $4::uuid, $5, $6, $7, $8, $9::date, $10::date, $11)
             {RETURNING}"
        ))
        .bind(empresa_id)
        .bind(&normalizada.nombre)
        .bind(&normalizada.tipo)
        .bind(normalizada.item_id.as_deref())
        .bind(&normalizada.sku)
        .bind(normalizada.cantidad_requerida)
        .bind(normalizada.cantidad_gratis)
        .bind(normalizada.descuento_porcentaje)
        .bind(&normalizada.fecha_desde)
        .bind(&normalizada.fecha_hasta)
        .bind(&normalizada.estado)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(serializar(&fila_a_json(&fila).map_err(interno)?)),
            None => Err(ApiError::dominio(
                "PROMOCION_NO_CREADA",
                "No se pudo crear la promoción.",
            )),
        }
    }

    pub async fn actualizar(
        &self,
        id: &str,
        empresa_id: &str,
        datos: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self
            .obtener(id, empresa_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Promoción", id))?;

        let mut combinada = Map::new();
        for clave in ["nombre", "tipo", "sku", "estado"] {
            let valor = datos
                .get(clave)
                .filter(|valor| !valor.is_null())
                .cloned()
                .or_else(|| actual.get(clave).cloned())
                .unwrap_or(Value::Null);
            combinada.insert(clave.to_string(), valor);
        }
        let item_id = datos
            .get("item_id")
            .filter(|valor| !valor.is_null())
            .cloned()
            .or_else(|| {
                actual
                    .get("item_id")
                    .filter(|valor| !valor.is_null())
                    .cloned()
            })
            .unwrap_or(Value::Null);
        combinada.insert("item_id".into(), item_id);
        for clave in [
            "cantidad_requerida",
            "cantidad_gratis",
            "descuento_porcentaje",
            "fecha_desde",
            "fecha_hasta",
        ] {
            let valor = match datos.get(clave) {
                Some(valor) => valor.clone(),
                None => actual.get(clave).cloned().unwrap_or(Value::Null),
            };
            combinada.insert(clave.to_string(), valor);
        }

        let normalizada = self
            .normalizar(empresa_id, &Value::Object(combinada))
            .await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "UPDATE pricing.promociones SET
               nombre = $3, tipo = $4, item_id = $5::uuid, sku = $6, cantidad_requerida = $7,
               cantidad_gratis = $8, descuento_porcentaje = $9, fecha_desde = $10::date,
               fecha_hasta = $11::date, estado = $12, actualizado_en = NOW()
             WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL
             {RETURNING}"
        ))
        .bind(id)
        .bind(empresa_id)
        .bind(&normalizada.nombre)
        .bind(&normalizada.tipo)
        .bind(normalizada.item_id.as_deref())
        .bind(&normalizada.sku)
        .bind(normalizada.cantidad_requerida)
        .bind(normalizada.cantidad_gratis)
        .bind(normalizada.descuento_porcentaje)
        .bind(&normalizada.fecha_desde)
        .bind(&normalizada.fecha_hasta)
        .bind(&normalizada.estado)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => Ok(serializar(&fila_a_json(&fila).map_err(interno)?)),
            None => Err(ApiError::no_encontrado("Promoción", id)),
        }
    }

    pub async fn eliminar(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE pricing.promociones SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Promoción", id));
        }
        Ok(())
    }

    async fn normalizar(&self, empresa_id: &str, datos: &Value) -> Result<Normalizada, ApiError> {
        let mut normalizada = validar(datos)?;
        let item_id = datos
            .get("item_id")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty());
        let sku = datos
            .get("sku")
            .and_then(Value::as_str)
            .filter(|valor| !valor.is_empty());
        if item_id.is_some() || sku.is_some() {
            let mut tx = self.db.transaccion().await.map_err(interno)?;
            let fila = match item_id {
                Some(item_id) => sqlx::query(
                    "SELECT id, codigo AS sku FROM catalog.items
                      WHERE empresa_id = $1::uuid AND tipo = 'producto' AND id = $2::uuid
                        AND eliminado_en IS NULL",
                )
                .bind(empresa_id)
                .bind(item_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?,
                None => sqlx::query(
                    "SELECT id, codigo AS sku FROM catalog.items
                      WHERE empresa_id = $1::uuid AND tipo = 'producto'
                        AND upper(codigo) = upper($2) AND eliminado_en IS NULL",
                )
                .bind(empresa_id)
                .bind(sku.unwrap_or(""))
                .fetch_optional(&mut *tx)
                .await
                .map_err(interno)?,
            };
            tx.commit().await.map_err(interno)?;
            let Some(fila) = fila else {
                return Err(ApiError::no_encontrado(
                    "Producto",
                    item_id.unwrap_or(sku.unwrap_or("")),
                ));
            };
            normalizada.item_id = Some(fila.get::<uuid::Uuid, _>("id").to_string());
            normalizada.sku = fila.get("sku");
        }
        Ok(normalizada)
    }
}

fn validar(datos: &Value) -> Result<Normalizada, ApiError> {
    let nombre = validar_nombre(&texto(datos, "nombre"))?;
    let tipo = datos
        .get("tipo")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if !TIPOS.contains(&tipo.as_str()) {
        return Err(ApiError::dominio(
            "TIPO_PROMOCION_INVALIDO",
            "El tipo de promoción no es válido.",
        ));
    }
    let mut cantidad_requerida = entero(datos.get("cantidad_requerida"));
    let mut cantidad_gratis = entero(datos.get("cantidad_gratis"));
    let descuento = entero(datos.get("descuento_porcentaje"));
    match tipo.as_str() {
        "2x1" => {
            cantidad_requerida = Some(2);
            cantidad_gratis = Some(1);
        }
        "regalo" => {
            if cantidad_requerida.unwrap_or(0) < 1 || cantidad_gratis.unwrap_or(0) < 1 {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "La promoción de regalo requiere cantidades.",
                ));
            }
        }
        "descuento_cantidad" => {
            if cantidad_requerida.unwrap_or(0) < 1 {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "Indica la cantidad requerida.",
                ));
            }
            if !descuento_valido(descuento) {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "El descuento debe estar entre 1 y 100.",
                ));
            }
        }
        _ => {
            if !descuento_valido(descuento) {
                return Err(ApiError::dominio(
                    "PROMOCION_INVALIDA",
                    "El descuento debe estar entre 1 y 100.",
                ));
            }
        }
    }
    let fecha_desde = fecha_corta(datos.get("fecha_desde"));
    let fecha_hasta = fecha_corta(datos.get("fecha_hasta"));
    if fecha_desde.is_empty() || fecha_hasta.is_empty() || fecha_hasta < fecha_desde {
        return Err(ApiError::dominio(
            "VIGENCIA_INVALIDA",
            "La vigencia de la promoción no es válida.",
        ));
    }
    Ok(Normalizada {
        nombre,
        tipo,
        item_id: None,
        sku: String::new(),
        cantidad_requerida,
        cantidad_gratis,
        descuento_porcentaje: descuento,
        fecha_desde,
        fecha_hasta,
        estado: if datos.get("estado").and_then(Value::as_str) == Some("inactivo") {
            "inactivo".to_string()
        } else {
            "activo".to_string()
        },
    })
}

fn serializar(fila: &Value) -> Value {
    json!({
        "id": fila.get("id").cloned().unwrap_or(Value::Null),
        "empresa_id": fila.get("empresa_id").cloned().unwrap_or(Value::Null),
        "nombre": fila.get("nombre").cloned().unwrap_or(Value::Null),
        "tipo": fila.get("tipo").cloned().unwrap_or(Value::Null),
        "item_id": fila.get("item_id").cloned().unwrap_or(Value::Null),
        "sku": fila.get("sku").and_then(Value::as_str).unwrap_or(""),
        "cantidad_requerida": fila.get("cantidad_requerida").cloned().unwrap_or(Value::Null),
        "cantidad_gratis": fila.get("cantidad_gratis").cloned().unwrap_or(Value::Null),
        "descuento_porcentaje": fila.get("descuento_porcentaje").cloned().unwrap_or(Value::Null),
        "fecha_desde": fecha_corta(fila.get("fecha_desde")),
        "fecha_hasta": fecha_corta(fila.get("fecha_hasta")),
        "estado": if fila.get("estado").and_then(Value::as_str) == Some("inactivo") {
            "inactivo"
        } else {
            "activo"
        },
        "creado_en": fila.get("creado_en").cloned().unwrap_or(Value::Null),
        "actualizado_en": fila.get("actualizado_en").cloned().unwrap_or(Value::Null),
    })
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn filtro_texto(filtros: &Value, clave: &str) -> Option<String> {
    match filtros.get(clave) {
        Some(Value::String(valor)) => Some(valor.clone()),
        Some(Value::Number(valor)) => Some(valor.to_string()),
        _ => None,
    }
}

fn entero(valor: Option<&Value>) -> Option<i64> {
    match valor? {
        Value::Number(numero) => numero.as_i64().or_else(|| {
            numero
                .as_f64()
                .filter(|valor| valor.is_finite() && valor.fract() == 0.0)
                .map(|valor| valor as i64)
        }),
        Value::String(texto) => texto.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn fecha_corta(valor: Option<&Value>) -> String {
    match valor {
        Some(Value::String(texto)) => texto.chars().take(10).collect(),
        Some(Value::Null) | None => String::new(),
        Some(otro) => otro.to_string().chars().take(10).collect(),
    }
}

fn descuento_valido(descuento: Option<i64>) -> bool {
    matches!(descuento, Some(valor) if (1..=100).contains(&valor))
}

fn validar_nombre(valor: &str) -> Result<String, ApiError> {
    let nombre = valor.trim().to_string();
    if !(2..=200).contains(&nombre.chars().count()) {
        return Err(ApiError::dominio(
            "NOMBRE_INVALIDO",
            "El nombre debe tener entre 2 y 200 caracteres.",
        ));
    }
    Ok(nombre)
}

fn es_uuid(valor: &str) -> bool {
    uuid::Uuid::parse_str(valor).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_y_normalizar_2x1() {
        let promo = validar(&json!({
            "nombre": "Promo 2x1",
            "tipo": "2x1",
            "fecha_desde": "2026-11-01",
            "fecha_hasta": "2026-11-30"
        }))
        .expect("promo");
        assert_eq!(promo.cantidad_requerida, Some(2));
        assert_eq!(promo.cantidad_gratis, Some(1));
    }

    #[test]
    fn debe_rechazar_vigencias_invertidas() {
        let error = validar(&json!({
            "nombre": "Promo",
            "tipo": "descuento_porcentaje",
            "descuento_porcentaje": 10,
            "fecha_desde": "2026-12-01",
            "fecha_hasta": "2026-11-01"
        }))
        .expect_err("vigencia");
        assert_eq!(error.codigo, "VIGENCIA_INVALIDA");
    }
}
