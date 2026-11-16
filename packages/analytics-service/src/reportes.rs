//! Definiciones y ejecución de reportes programados.
//!
//! El servicio guarda la programación y el contenido generado. La entrega al
//! destinatario (correo, webhook o frontend) queda fuera de este módulo: aquí
//! solo se calcula y se persiste el resultado para que otro consumidor lo lea.

use crate::analitica;
use crate::db::{error_db, Db};
use crate::errors::ApiError;
use chrono::{DateTime, Duration, Months, Utc};
use serde_json::Value;
use sqlx::Row;

#[derive(Debug, Clone)]
pub struct NuevoReporte {
    pub nombre: String,
    pub tipo: String,
    pub formato: String,
    pub frecuencia: String,
    pub destino: String,
    pub proxima_ejecucion: String,
    pub negocio_id: Option<String>,
}

pub async fn listar(db: &Db, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
    let filas = sqlx::query(
        "SELECT to_jsonb(r) AS datos
           FROM analytics.reportes_programados r
          WHERE r.empresa_id = $1::uuid
          ORDER BY r.creado_en DESC",
    )
    .bind(empresa_id)
    .fetch_all(db.pool())
    .await
    .map_err(error_db)?;
    filas
        .iter()
        .map(|fila| fila.try_get("datos").map_err(error_db))
        .collect()
}

pub async fn crear(
    db: &Db,
    empresa_id: &str,
    creado_por: &str,
    input: NuevoReporte,
) -> Result<Value, ApiError> {
    let fila = sqlx::query(
        "INSERT INTO analytics.reportes_programados
           (empresa_id, negocio_id, nombre, tipo, formato, frecuencia,
            destino, proxima_ejecucion, creado_por)
         VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8::timestamptz, $9::uuid)
         RETURNING to_jsonb(reportes_programados) AS datos",
    )
    .bind(empresa_id)
    .bind(input.negocio_id.as_deref())
    .bind(&input.nombre)
    .bind(&input.tipo)
    .bind(&input.formato)
    .bind(&input.frecuencia)
    .bind(&input.destino)
    .bind(&input.proxima_ejecucion)
    .bind(creado_por)
    .fetch_optional(db.pool())
    .await
    .map_err(error_db)?;
    fila.ok_or_else(|| {
        ApiError::dominio(
            "REPORTE_NO_CREADO",
            "No se pudo crear el reporte programado.",
        )
    })?
    .try_get("datos")
    .map_err(error_db)
}

pub async fn cambiar_estado(
    db: &Db,
    empresa_id: &str,
    id: &str,
    activo: bool,
) -> Result<Value, ApiError> {
    let fila = sqlx::query(
        "UPDATE analytics.reportes_programados
            SET activo = $1, actualizado_en = NOW()
          WHERE id = $2::uuid AND empresa_id = $3::uuid
          RETURNING to_jsonb(reportes_programados) AS datos",
    )
    .bind(activo)
    .bind(id)
    .bind(empresa_id)
    .fetch_optional(db.pool())
    .await
    .map_err(error_db)?;
    fila.ok_or_else(|| ApiError::no_encontrado("Reporte", id))?
        .try_get("datos")
        .map_err(error_db)
}

pub async fn ejecuciones(db: &Db, empresa_id: &str, id: &str) -> Result<Vec<Value>, ApiError> {
    let filas = sqlx::query(
        "SELECT to_jsonb(e) AS datos
           FROM analytics.reporte_ejecuciones e
          WHERE e.reporte_id = $1::uuid AND e.empresa_id = $2::uuid
          ORDER BY e.ejecutada_en DESC",
    )
    .bind(id)
    .bind(empresa_id)
    .fetch_all(db.pool())
    .await
    .map_err(error_db)?;
    filas
        .iter()
        .map(|fila| fila.try_get("datos").map_err(error_db))
        .collect()
}

/// Calcula y guarda una ejecución del reporte. Devuelve la ejecución creada.
pub async fn ejecutar(db: &Db, empresa_id: &str, id: &str) -> Result<Value, ApiError> {
    let definicion = sqlx::query(
        "SELECT id, negocio_id, tipo, formato, frecuencia, proxima_ejecucion
           FROM analytics.reportes_programados
          WHERE id = $1::uuid AND empresa_id = $2::uuid",
    )
    .bind(id)
    .bind(empresa_id)
    .fetch_optional(db.pool())
    .await
    .map_err(error_db)?
    .ok_or_else(|| ApiError::no_encontrado("Reporte", id))?;
    let negocio_id = definicion
        .try_get::<Option<uuid::Uuid>, _>("negocio_id")
        .map_err(error_db)?
        .map(|valor| valor.to_string());
    let tipo: String = definicion.try_get("tipo").map_err(error_db)?;
    let formato: String = definicion.try_get("formato").map_err(error_db)?;
    let frecuencia: String = definicion.try_get("frecuencia").map_err(error_db)?;
    let proxima: DateTime<Utc> = definicion.try_get("proxima_ejecucion").map_err(error_db)?;

    let (contenido, error) =
        match calcular(db, empresa_id, negocio_id.as_deref(), &tipo, &formato).await {
            Ok(contenido) => (contenido, String::new()),
            Err(err) => (String::new(), err.mensaje),
        };
    let estado = if error.is_empty() { "ok" } else { "error" };
    let siguiente = siguiente_ejecucion(proxima, &frecuencia);

    let mut tx = db.transaccion().await.map_err(error_db)?;
    let fila = sqlx::query(
        "INSERT INTO analytics.reporte_ejecuciones
           (reporte_id, empresa_id, negocio_id, tipo, formato, estado, contenido, error)
         VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8)
         RETURNING to_jsonb(reporte_ejecuciones) AS datos",
    )
    .bind(id)
    .bind(empresa_id)
    .bind(negocio_id.as_deref())
    .bind(&tipo)
    .bind(&formato)
    .bind(estado)
    .bind(&contenido)
    .bind(&error)
    .fetch_one(&mut *tx)
    .await
    .map_err(error_db)?;
    sqlx::query(
        "UPDATE analytics.reportes_programados
            SET ultima_ejecucion = NOW(),
                proxima_ejecucion = $1,
                actualizado_en = NOW()
          WHERE id = $2::uuid AND empresa_id = $3::uuid",
    )
    .bind(siguiente)
    .bind(id)
    .bind(empresa_id)
    .execute(&mut *tx)
    .await
    .map_err(error_db)?;
    tx.commit().await.map_err(error_db)?;
    fila.try_get("datos").map_err(error_db)
}

/// Ejecuta los reportes vencidos. Devuelve la cantidad procesada.
pub async fn ejecutar_vencidos(db: &Db, limite: i64) -> Result<usize, ApiError> {
    let filas = sqlx::query(
        "SELECT id, empresa_id
           FROM analytics.reportes_programados
          WHERE activo AND proxima_ejecucion <= NOW()
          ORDER BY proxima_ejecucion
          LIMIT $1",
    )
    .bind(limite)
    .fetch_all(db.pool())
    .await
    .map_err(error_db)?;
    let mut procesados = 0usize;
    for fila in filas {
        let id: uuid::Uuid = fila.try_get("id").map_err(error_db)?;
        let empresa_id: uuid::Uuid = fila.try_get("empresa_id").map_err(error_db)?;
        if ejecutar(db, &empresa_id.to_string(), &id.to_string())
            .await
            .is_ok()
        {
            procesados += 1;
        }
    }
    Ok(procesados)
}

async fn calcular(
    db: &Db,
    empresa_id: &str,
    negocio_id: Option<&str>,
    tipo: &str,
    formato: &str,
) -> Result<String, ApiError> {
    let resumen = if tipo == "cohortes" {
        analitica::cohortes_negocios(db, empresa_id, negocio_id, None, None).await?
    } else {
        analitica::resumen_negocios(db, empresa_id, negocio_id, None, None).await?
    };
    if formato == "csv" {
        return Ok(if tipo == "cohortes" {
            analitica::cohortes_csv(&resumen)
        } else {
            analitica::resumen_csv(&resumen)
        });
    }
    serde_json::to_string_pretty(&resumen).map_err(|_| {
        ApiError::dominio(
            "REPORTE_NO_SERIALIZABLE",
            "No se pudo serializar el reporte.",
        )
    })
}

fn siguiente_ejecucion(base: DateTime<Utc>, frecuencia: &str) -> DateTime<Utc> {
    match frecuencia {
        "diaria" => base + Duration::days(1),
        "semanal" => base + Duration::days(7),
        _ => base
            .checked_add_months(Months::new(1))
            .unwrap_or_else(|| base + Duration::days(30)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_avanzar_la_proxima_ejecucion_segun_frecuencia() {
        let base = DateTime::parse_from_rfc3339("2026-01-31T06:00:00Z")
            .expect("fecha")
            .with_timezone(&Utc);
        assert_eq!(
            siguiente_ejecucion(base, "diaria"),
            base + Duration::days(1)
        );
        assert_eq!(
            siguiente_ejecucion(base, "semanal"),
            base + Duration::days(7)
        );
        assert_eq!(
            siguiente_ejecucion(base, "mensual"),
            DateTime::parse_from_rfc3339("2026-02-28T06:00:00Z")
                .expect("fecha")
                .with_timezone(&Utc)
        );
    }
}
