//! Cupones de descuento por empresa.
//!
//! El cupón es una regla de pricing independiente de las promociones por item.
//! Se aplica al total de la orden y su canje es idempotente por orden.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::paginacion::{crear_pagina, parsear_paginacion};
use serde_json::Value;
use sqlx::Row;
use std::sync::Arc;

const COLUMNAS: &str = "id, empresa_id, codigo, nombre, tipo, valor,
  minimo_compra_cents, maximo_descuento_cents, usos_maximos, usos_actuales,
  fecha_desde::text AS fecha_desde, fecha_hasta::text AS fecha_hasta,
  activo, creado_en, actualizado_en";
const RETURNING: &str = "RETURNING id, empresa_id, codigo, nombre, tipo, valor,
  minimo_compra_cents, maximo_descuento_cents, usos_maximos, usos_actuales,
  fecha_desde::text AS fecha_desde, fecha_hasta::text AS fecha_hasta,
  activo, creado_en, actualizado_en";

#[derive(Debug, Clone, PartialEq)]
pub struct CuponValidado {
    pub cupon_id: String,
    pub codigo: String,
    pub descuento_cents: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct Normalizada {
    codigo: String,
    nombre: String,
    tipo: String,
    valor: i64,
    minimo_compra_cents: i64,
    maximo_descuento_cents: Option<i64>,
    usos_maximos: Option<i64>,
    fecha_desde: String,
    fecha_hasta: String,
    activo: bool,
}

pub struct Cupones {
    db: Arc<Db>,
}

impl Cupones {
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
        if let Some(activo) = filtros.get("activo").and_then(Value::as_bool) {
            params.push(activo.to_string());
            condiciones.push(format!("activo = ${}::boolean", params.len()));
        }
        let where_clause = format!("WHERE {}", condiciones.join(" AND "));
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let sql_total = format!("SELECT COUNT(*)::int AS n FROM pricing.cupones {where_clause}");
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
               FROM pricing.cupones {where_clause}
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
        let cupones = filas
            .iter()
            .map(|fila| fila_a_json(fila).map_err(interno))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(crear_pagina(cupones, total as i64, paginacion))
    }

    pub async fn obtener(&self, id: &str, empresa_id: &str) -> Result<Option<Value>, ApiError> {
        if !es_uuid(id) {
            return Ok(None);
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS} FROM pricing.cupones
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL"
        ))
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        fila.map(|fila| fila_a_json(&fila).map_err(interno))
            .transpose()
    }

    pub async fn crear(&self, empresa_id: &str, datos: &Value) -> Result<Value, ApiError> {
        let normalizada = normalizar(datos)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "INSERT INTO pricing.cupones
               (empresa_id, codigo, nombre, tipo, valor, minimo_compra_cents,
                maximo_descuento_cents, usos_maximos, fecha_desde, fecha_hasta, activo)
             VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9::date, $10::date, $11)
             {RETURNING}"
        ))
        .bind(empresa_id)
        .bind(&normalizada.codigo)
        .bind(&normalizada.nombre)
        .bind(&normalizada.tipo)
        .bind(normalizada.valor as i32)
        .bind(normalizada.minimo_compra_cents as i32)
        .bind(normalizada.maximo_descuento_cents.map(|valor| valor as i32))
        .bind(normalizada.usos_maximos.map(|valor| valor as i32))
        .bind(&normalizada.fecha_desde)
        .bind(&normalizada.fecha_hasta)
        .bind(normalizada.activo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?
        .ok_or_else(|| ApiError::conflicto("No se pudo crear el cupón."))?;
        tx.commit().await.map_err(interno)?;
        fila_a_json(&fila).map_err(interno)
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
            .ok_or_else(|| ApiError::no_encontrado("Cupon", id))?;
        let mut combinado = actual.clone();
        if let Some(objeto) = combinado.as_object_mut() {
            for (clave, valor) in datos.as_object().into_iter().flatten() {
                objeto.insert(clave.clone(), valor.clone());
            }
        }
        let normalizada = normalizar(&combinado)?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "UPDATE pricing.cupones
                SET codigo = $3, nombre = $4, tipo = $5, valor = $6,
                    minimo_compra_cents = $7, maximo_descuento_cents = $8,
                    usos_maximos = $9, fecha_desde = $10::date,
                    fecha_hasta = $11::date, activo = $12, actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL
              {RETURNING}"
        ))
        .bind(id)
        .bind(empresa_id)
        .bind(&normalizada.codigo)
        .bind(&normalizada.nombre)
        .bind(&normalizada.tipo)
        .bind(normalizada.valor as i32)
        .bind(normalizada.minimo_compra_cents as i32)
        .bind(normalizada.maximo_descuento_cents.map(|valor| valor as i32))
        .bind(normalizada.usos_maximos.map(|valor| valor as i32))
        .bind(&normalizada.fecha_desde)
        .bind(&normalizada.fecha_hasta)
        .bind(normalizada.activo)
        .fetch_optional(&mut *tx)
        .await
        .map_err(conflicto_o_interno)?
        .ok_or_else(|| ApiError::no_encontrado("Cupon", id))?;
        tx.commit().await.map_err(interno)?;
        fila_a_json(&fila).map_err(interno)
    }

    pub async fn eliminar(&self, id: &str, empresa_id: &str) -> Result<(), ApiError> {
        let fila = sqlx::query(
            "UPDATE pricing.cupones
                SET eliminado_en = NOW(), actualizado_en = NOW()
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL
              RETURNING id",
        )
        .bind(id)
        .bind(empresa_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(interno)?;
        if fila.is_none() {
            return Err(ApiError::no_encontrado("Cupon", id));
        }
        Ok(())
    }

    /// Valida un cupón y calcula el descuento sin consumir usos.
    pub async fn validar(
        &self,
        empresa_id: &str,
        codigo: &str,
        monto_cents: i64,
    ) -> Result<CuponValidado, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = buscar(&mut tx, empresa_id, codigo).await?;
        tx.commit().await.map_err(interno)?;
        validar_fila(&fila, monto_cents)
    }

    /// Consume un uso del cupón. Idempotente por `(cupon, orden)`.
    pub async fn canjear(
        &self,
        empresa_id: &str,
        codigo: &str,
        order_id: &str,
        monto_cents: i64,
    ) -> Result<CuponValidado, ApiError> {
        if !es_uuid(order_id) {
            return Err(ApiError::dominio(
                "ORDEN_INVALIDA",
                "El id de la orden debe ser un UUID.",
            ));
        }
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = buscar(&mut tx, empresa_id, codigo).await?;
        let validado = validar_fila(&fila, monto_cents)?;
        let existente: Option<i32> = sqlx::query_scalar(
            "SELECT monto_descuento_cents FROM pricing.cupon_usos
              WHERE cupon_id = $1::uuid AND order_id = $2::uuid AND estado = 'usado'",
        )
        .bind(&validado.cupon_id)
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if let Some(monto) = existente {
            tx.commit().await.map_err(interno)?;
            return Ok(CuponValidado {
                descuento_cents: monto as i64,
                ..validado
            });
        }
        sqlx::query(
            "INSERT INTO pricing.cupon_usos
               (cupon_id, empresa_id, order_id, codigo, monto_descuento_cents, estado)
             VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, 'usado')",
        )
        .bind(&validado.cupon_id)
        .bind(empresa_id)
        .bind(order_id)
        .bind(&validado.codigo)
        .bind(validado.descuento_cents as i32)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        sqlx::query(
            "UPDATE pricing.cupones SET usos_actuales = usos_actuales + 1,
                    actualizado_en = NOW() WHERE id = $1::uuid",
        )
        .bind(&validado.cupon_id)
        .execute(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        Ok(validado)
    }

    /// Libera un canje si la orden no llegó a persistirse. Idempotente.
    pub async fn liberar(
        &self,
        empresa_id: &str,
        codigo: &str,
        order_id: &str,
    ) -> Result<(), ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "UPDATE pricing.cupon_usos u
                SET estado = 'liberado'
               FROM pricing.cupones c
              WHERE u.cupon_id = c.id AND u.empresa_id = $1::uuid
                AND upper(c.codigo) = upper($2) AND u.order_id = $3::uuid
                AND u.estado = 'usado'
              RETURNING u.cupon_id",
        )
        .bind(empresa_id)
        .bind(codigo)
        .bind(order_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if let Some(fila) = fila {
            let cupon_id: uuid::Uuid = fila.get("cupon_id");
            sqlx::query(
                "UPDATE pricing.cupones
                    SET usos_actuales = GREATEST(usos_actuales - 1, 0), actualizado_en = NOW()
                  WHERE id = $1::uuid",
            )
            .bind(cupon_id)
            .execute(&mut *tx)
            .await
            .map_err(interno)?;
        }
        tx.commit().await.map_err(interno)?;
        Ok(())
    }
}

async fn buscar(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa_id: &str,
    codigo: &str,
) -> Result<sqlx::postgres::PgRow, ApiError> {
    let codigo = codigo.trim();
    if codigo.is_empty() {
        return Err(ApiError::dominio(
            "CUPON_INVALIDO",
            "El código del cupón es obligatorio.",
        ));
    }
    sqlx::query(&format!(
        "SELECT {COLUMNAS} FROM pricing.cupones
          WHERE empresa_id = $1::uuid AND upper(codigo) = upper($2)
            AND eliminado_en IS NULL
          FOR UPDATE"
    ))
    .bind(empresa_id)
    .bind(codigo)
    .fetch_optional(&mut **tx)
    .await
    .map_err(interno)?
    .ok_or_else(|| ApiError::dominio("CUPON_NO_ENCONTRADO", "El cupón no existe."))
}

fn validar_fila(fila: &sqlx::postgres::PgRow, monto_cents: i64) -> Result<CuponValidado, ApiError> {
    let activo: bool = fila.try_get("activo").map_err(interno)?;
    if !activo {
        return Err(ApiError::dominio(
            "CUPON_INACTIVO",
            "El cupón está inactivo.",
        ));
    }
    let desde: String = fila.try_get("fecha_desde").map_err(interno)?;
    let hasta: String = fila.try_get("fecha_hasta").map_err(interno)?;
    let hoy = chrono::Local::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    if hoy < desde || hoy > hasta {
        return Err(ApiError::dominio(
            "CUPON_VENCIDO",
            "El cupón no está vigente.",
        ));
    }
    let minimo: i32 = fila.try_get("minimo_compra_cents").map_err(interno)?;
    if monto_cents < i64::from(minimo) {
        return Err(ApiError::dominio(
            "CUPON_MINIMO",
            "El monto de la orden no alcanza el mínimo del cupón.",
        ));
    }
    let usos_maximos: Option<i32> = fila.try_get("usos_maximos").map_err(interno)?;
    let usos_actuales: i32 = fila.try_get("usos_actuales").map_err(interno)?;
    if usos_maximos.is_some_and(|maximo| usos_actuales >= maximo) {
        return Err(ApiError::dominio(
            "CUPON_AGOTADO",
            "El cupón agotó sus usos.",
        ));
    }
    let tipo: String = fila.try_get("tipo").map_err(interno)?;
    let valor: i32 = fila.try_get("valor").map_err(interno)?;
    let tope: Option<i32> = fila.try_get("maximo_descuento_cents").map_err(interno)?;
    let mut descuento = match tipo.as_str() {
        "porcentaje" => ((monto_cents as f64 * f64::from(valor)) / 100.0).round() as i64,
        _ => i64::from(valor),
    };
    if let Some(tope) = tope {
        descuento = descuento.min(i64::from(tope));
    }
    descuento = descuento.clamp(0, monto_cents);
    Ok(CuponValidado {
        cupon_id: fila
            .try_get::<uuid::Uuid, _>("id")
            .map(|id| id.to_string())
            .map_err(interno)?,
        codigo: fila.try_get("codigo").map_err(interno)?,
        descuento_cents: descuento,
    })
}

fn normalizar(datos: &Value) -> Result<Normalizada, ApiError> {
    let codigo = texto(datos, "codigo").to_uppercase();
    if !(3..=32).contains(&codigo.len())
        || !codigo
            .chars()
            .all(|caracter| caracter.is_ascii_alphanumeric() || caracter == '_' || caracter == '-')
    {
        return Err(ApiError::dominio(
            "CODIGO_CUPON_INVALIDO",
            "El código debe tener 3-32 caracteres alfanuméricos, guion o guion bajo.",
        ));
    }
    let nombre = texto(datos, "nombre");
    if !(2..=120).contains(&nombre.chars().count()) {
        return Err(ApiError::dominio(
            "NOMBRE_CUPON_INVALIDO",
            "El nombre del cupón debe tener entre 2 y 120 caracteres.",
        ));
    }
    let tipo = texto(datos, "tipo");
    if !["porcentaje", "monto_fijo"].contains(&tipo.as_str()) {
        return Err(ApiError::dominio(
            "TIPO_CUPON_INVALIDO",
            "El tipo debe ser porcentaje o monto_fijo.",
        ));
    }
    let valor = entero(datos.get("valor")).unwrap_or(0);
    if valor <= 0 || (tipo == "porcentaje" && valor > 100) {
        return Err(ApiError::dominio(
            "VALOR_CUPON_INVALIDO",
            "El valor del cupón no es válido para su tipo.",
        ));
    }
    let fecha_desde = fecha(datos.get("fecha_desde"));
    let fecha_hasta = fecha(datos.get("fecha_hasta"));
    if fecha_desde.is_empty() || fecha_hasta.is_empty() || fecha_hasta < fecha_desde {
        return Err(ApiError::dominio(
            "VIGENCIA_CUPON_INVALIDA",
            "La vigencia del cupón no es válida.",
        ));
    }
    Ok(Normalizada {
        codigo,
        nombre,
        tipo,
        valor,
        minimo_compra_cents: entero(datos.get("minimo_compra_cents")).unwrap_or(0).max(0),
        maximo_descuento_cents: entero(datos.get("maximo_descuento_cents")).filter(|v| *v > 0),
        usos_maximos: entero(datos.get("usos_maximos")).filter(|v| *v > 0),
        fecha_desde,
        fecha_hasta,
        activo: datos.get("activo").and_then(Value::as_bool).unwrap_or(true),
    })
}

fn es_uuid(valor: &str) -> bool {
    uuid::Uuid::parse_str(valor).is_ok()
}

fn texto(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

fn entero(valor: Option<&Value>) -> Option<i64> {
    valor.and_then(|valor| {
        valor.as_i64().or_else(|| {
            valor
                .as_f64()
                .filter(|numero| numero.fract() == 0.0)
                .map(|n| n as i64)
        })
    })
}

fn fecha(valor: Option<&Value>) -> String {
    valor
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .chars()
        .take(10)
        .collect()
}

fn filtro_texto(filtros: &Value, clave: &str) -> Option<String> {
    match filtros.get(clave) {
        Some(Value::String(valor)) => Some(valor.clone()),
        Some(Value::Number(valor)) => Some(valor.to_string()),
        _ => None,
    }
}

fn conflicto_o_interno(error: sqlx::Error) -> ApiError {
    if let Some(codigo) = error.as_database_error().and_then(|db| db.code()) {
        if codigo == "23505" {
            return ApiError::conflicto("Ya existe un cupón con ese código.");
        }
    }
    interno(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn debe_normalizar_un_cupon_porcentual() {
        let cupon = normalizar(&json!({
            "codigo": "verano-25",
            "nombre": "Verano 25",
            "tipo": "porcentaje",
            "valor": 25,
            "fecha_desde": "2026-01-01",
            "fecha_hasta": "2026-12-31"
        }))
        .expect("cupon");
        assert_eq!(cupon.codigo, "VERANO-25");
        assert_eq!(cupon.valor, 25);
    }

    #[test]
    fn debe_rechazar_valores_y_vigencias_invalidas() {
        let error = normalizar(&json!({
            "codigo": "X",
            "nombre": "Corto",
            "tipo": "porcentaje",
            "valor": 150,
            "fecha_desde": "2026-01-01",
            "fecha_hasta": "2025-01-01"
        }))
        .expect_err("invalido");
        assert_eq!(error.codigo, "CODIGO_CUPON_INVALIDO");
    }
}
