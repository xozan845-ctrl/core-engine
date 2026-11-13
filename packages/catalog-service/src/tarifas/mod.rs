//! Tarifas por fecha/rango (Fase C, ADR-0001).
//!
//! Réplica de `items/tarifas.service.ts`. Cualquier item puede definir precios
//! por temporada; la cotización agrupa los días consecutivos con el mismo
//! precio y usa el precio base cuando no hay tarifa. Es la pieza de precio para
//! hotel/alquiler; la disponibilidad sigue en scheduling (recursos + citas por
//! rango).

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::money::Money;
use chrono::{Duration, NaiveDate};
use serde_json::{json, Value};
use sqlx::postgres::PgRow;
use std::sync::Arc;

const MAX_DIAS: i64 = 366;

const COLUMNAS: &str = "id, empresa_id, item_id, desde::text AS desde,
  hasta::text AS hasta, precio_cents, creado_en, actualizado_en";

const MENSAJE_CONFLICTO: &str = "La tarifa se solapa con otra del mismo item.";

pub struct Tarifas {
    db: Arc<Db>,
}

impl Tarifas {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    pub async fn listar(&self, empresa_id: &str, item_id: &str) -> Result<Value, ApiError> {
        self.verificar_item(empresa_id, item_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS}
               FROM catalog.item_tarifas
              WHERE empresa_id = $1::uuid AND item_id = $2::uuid
              ORDER BY desde"
        ))
        .bind(empresa_id)
        .bind(item_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        filas_json(&filas)
    }

    pub async fn crear(
        &self,
        empresa_id: &str,
        item_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        self.verificar_item(empresa_id, item_id).await?;
        let rango = validar_rango(input)?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            let fila = sqlx::query(&format!(
                "INSERT INTO catalog.item_tarifas (empresa_id, item_id, desde, hasta, precio_cents)
                 VALUES ($1::uuid, $2::uuid, $3::date, $4::date, $5::bigint)
                 RETURNING {COLUMNAS}"
            ))
            .bind(empresa_id)
            .bind(item_id)
            .bind(&rango.desde)
            .bind(&rango.hasta)
            .bind(rango.precio)
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                Err(ApiError::dominio(
                    "TARIFA_NO_CREADA",
                    "No se pudo guardar la tarifa.",
                ))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error))
            }
        }
    }

    pub async fn actualizar(
        &self,
        empresa_id: &str,
        item_id: &str,
        tarifa_id: &str,
        input: &Value,
    ) -> Result<Value, ApiError> {
        let actual = self.buscar(empresa_id, item_id, tarifa_id).await?;

        let desde = input
            .get("desde")
            .filter(|valor| !valor.is_null())
            .cloned()
            .unwrap_or_else(|| actual.get("desde").cloned().unwrap_or(Value::Null));
        let hasta = input
            .get("hasta")
            .filter(|valor| !valor.is_null())
            .cloned()
            .unwrap_or_else(|| actual.get("hasta").cloned().unwrap_or(Value::Null));
        let precio = input
            .get("precio_cents")
            .filter(|valor| !valor.is_null())
            .cloned()
            .unwrap_or_else(|| actual.get("precio_cents").cloned().unwrap_or(Value::Null));
        let combinado = json!({
            "desde": desde,
            "hasta": hasta,
            "precio_cents": precio,
        });
        let rango = validar_rango(&combinado)?;

        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let resultado: Result<Option<PgRow>, sqlx::Error> = async {
            let fila = sqlx::query(&format!(
                "UPDATE catalog.item_tarifas
                    SET desde = $1::date, hasta = $2::date, precio_cents = $3::bigint,
                        actualizado_en = NOW()
                  WHERE id = $4::uuid AND empresa_id = $5::uuid AND item_id = $6::uuid
                  RETURNING {COLUMNAS}"
            ))
            .bind(&rango.desde)
            .bind(&rango.hasta)
            .bind(rango.precio)
            .bind(tarifa_id)
            .bind(empresa_id)
            .bind(item_id)
            .fetch_optional(&mut *tx)
            .await?;
            Ok(fila)
        }
        .await;

        match resultado {
            Ok(Some(fila)) => {
                tx.commit().await.map_err(interno)?;
                fila_json(&fila)
            }
            Ok(None) => {
                let _ = tx.rollback().await;
                Err(ApiError::no_encontrado("Tarifa", tarifa_id))
            }
            Err(error) => {
                let _ = tx.rollback().await;
                Err(error_conflicto(error))
            }
        }
    }

    pub async fn eliminar(
        &self,
        empresa_id: &str,
        item_id: &str,
        tarifa_id: &str,
    ) -> Result<Value, ApiError> {
        self.buscar(empresa_id, item_id, tarifa_id).await?;
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "DELETE FROM catalog.item_tarifas
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND item_id = $3::uuid
              RETURNING id",
        )
        .bind(tarifa_id)
        .bind(empresa_id)
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        if fila.is_none() {
            let _ = tx.rollback().await;
            return Err(ApiError::no_encontrado("Tarifa", tarifa_id));
        }
        tx.commit().await.map_err(interno)?;
        Ok(json!({ "ok": true }))
    }

    /// Cotiza un rango inclusivo de fechas. Cada día usa la primera tarifa que
    /// lo contiene y, si no hay, el precio base del item.
    pub async fn cotizacion(
        &self,
        empresa_id: &str,
        item_id: &str,
        desde: &Value,
        hasta: &Value,
    ) -> Result<Value, ApiError> {
        let item = self.verificar_item(empresa_id, item_id).await?;
        let desde = parse_fecha(desde);
        let hasta = parse_fecha(hasta);
        let (Some(desde), Some(hasta)) = (desde, hasta) else {
            return Err(ApiError::dominio(
                "RANGO_INVALIDO",
                "Usa fechas YYYY-MM-DD.",
            ));
        };
        let dias = (hasta - desde).num_days() + 1;
        if dias < 1 {
            return Err(ApiError::dominio(
                "RANGO_INVALIDO",
                "La fecha final no puede ser anterior.",
            ));
        }
        if dias > MAX_DIAS {
            return Err(ApiError::dominio(
                "RANGO_INVALIDO",
                format!("El rango admite hasta {MAX_DIAS} dias."),
            ));
        }

        let desde_texto = formatear(desde);
        let hasta_texto = formatear(hasta);
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let filas = sqlx::query(&format!(
            "SELECT {COLUMNAS}
               FROM catalog.item_tarifas
              WHERE empresa_id = $1::uuid AND item_id = $2::uuid
                AND daterange(desde, hasta, '[]') && daterange($3::date, $4::date, '[]')
              ORDER BY desde"
        ))
        .bind(empresa_id)
        .bind(item_id)
        .bind(&desde_texto)
        .bind(&hasta_texto)
        .fetch_all(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;

        let tarifas = filas
            .iter()
            .map(|fila| {
                let valor = fila_a_json(fila).map_err(interno)?;
                Ok(TarifaRango {
                    desde: texto_de(&valor, "desde"),
                    hasta: texto_de(&valor, "hasta"),
                    precio_cents: entero_de(&valor, "precio_cents").unwrap_or(0),
                })
            })
            .collect::<Result<Vec<_>, ApiError>>()?;

        let (desglose, total) = agrupar_desglose(desde, dias, &tarifas, item.precio_cents)?;
        Ok(json!({
            "item_id": item_id,
            "desde": desde_texto,
            "hasta": hasta_texto,
            "dias": dias,
            "total_cents": total,
            "desglose": desglose,
        }))
    }

    async fn verificar_item(
        &self,
        empresa_id: &str,
        item_id: &str,
    ) -> Result<ItemPrecio, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(
            "SELECT id, tipo, precio_cents FROM catalog.items
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND eliminado_en IS NULL",
        )
        .bind(item_id)
        .bind(empresa_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        let fila = fila.ok_or_else(|| ApiError::no_encontrado("Item", item_id))?;
        let valor = fila_json(&fila)?;
        Ok(ItemPrecio {
            precio_cents: entero_de(&valor, "precio_cents").unwrap_or(0),
        })
    }

    async fn buscar(
        &self,
        empresa_id: &str,
        item_id: &str,
        tarifa_id: &str,
    ) -> Result<Value, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(interno)?;
        let fila = sqlx::query(&format!(
            "SELECT {COLUMNAS}
               FROM catalog.item_tarifas
              WHERE id = $1::uuid AND empresa_id = $2::uuid AND item_id = $3::uuid"
        ))
        .bind(tarifa_id)
        .bind(empresa_id)
        .bind(item_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(interno)?;
        tx.commit().await.map_err(interno)?;
        match fila {
            Some(fila) => fila_json(&fila),
            None => Err(ApiError::no_encontrado("Tarifa", tarifa_id)),
        }
    }
}

struct ItemPrecio {
    precio_cents: i64,
}

struct TarifaRango {
    desde: String,
    hasta: String,
    precio_cents: i64,
}

#[derive(Debug)]
struct RangoTarifa {
    desde: String,
    hasta: String,
    precio: i64,
}

fn filas_json(filas: &[PgRow]) -> Result<Value, ApiError> {
    let valores = filas
        .iter()
        .map(fila_a_json)
        .collect::<Result<Vec<_>, _>>()
        .map_err(interno)?;
    Ok(Value::Array(valores))
}

fn fila_json(fila: &PgRow) -> Result<Value, ApiError> {
    fila_a_json(fila).map_err(interno)
}

fn error_conflicto(error: sqlx::Error) -> ApiError {
    if es_conflicto_de_rango(&error) {
        ApiError::conflicto(MENSAJE_CONFLICTO)
    } else {
        interno(error)
    }
}

fn es_conflicto_de_rango(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|base| base.code())
        .is_some_and(|codigo| codigo == "23P01" || codigo == "23505")
}

fn texto_de(valor: &Value, clave: &str) -> String {
    valor
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn entero_de(valor: &Value, clave: &str) -> Option<i64> {
    valor.get(clave).and_then(Value::as_i64)
}

fn parse_fecha(valor: &Value) -> Option<NaiveDate> {
    let texto = valor.as_str()?;
    let bytes = texto.as_bytes();
    if bytes.len() != 10 {
        return None;
    }
    for (indice, byte) in bytes.iter().enumerate() {
        if matches!(indice, 4 | 7) {
            if *byte != b'-' {
                return None;
            }
        } else if !byte.is_ascii_digit() {
            return None;
        }
    }
    NaiveDate::parse_from_str(texto, "%Y-%m-%d").ok()
}

fn formatear(fecha: NaiveDate) -> String {
    fecha.format("%Y-%m-%d").to_string()
}

fn validar_rango(input: &Value) -> Result<RangoTarifa, ApiError> {
    let desde = parse_fecha(input.get("desde").unwrap_or(&Value::Null));
    let hasta = parse_fecha(input.get("hasta").unwrap_or(&Value::Null));
    let (Some(desde), Some(hasta)) = (desde, hasta) else {
        return Err(ApiError::dominio(
            "RANGO_INVALIDO",
            "Usa fechas YYYY-MM-DD.",
        ));
    };
    if hasta < desde {
        return Err(ApiError::dominio(
            "RANGO_INVALIDO",
            "La fecha final no puede ser anterior.",
        ));
    }
    let Some(precio) = input.get("precio_cents") else {
        return Err(ApiError::dominio(
            "PRECIO_INVALIDO",
            "El precio debe ser un entero no negativo.",
        ));
    };
    let precio = validar_precio(precio)?;
    Ok(RangoTarifa {
        desde: formatear(desde),
        hasta: formatear(hasta),
        precio,
    })
}

fn validar_precio(valor: &Value) -> Result<i64, ApiError> {
    let precio = numero_entero(valor).filter(|valor| *valor >= 0);
    let Some(precio) = precio else {
        return Err(ApiError::dominio(
            "PRECIO_INVALIDO",
            "El precio debe ser un entero no negativo.",
        ));
    };
    Money::desde_centavos(precio)
        .map(|monto| monto.centavos)
        .map_err(|_| {
            ApiError::dominio(
                "PRECIO_INVALIDO",
                "El precio debe ser un entero no negativo.",
            )
        })
}

fn numero_entero(valor: &Value) -> Option<i64> {
    let numero = match valor {
        Value::Number(numero) => numero.as_f64()?,
        Value::String(texto) => {
            let texto = texto.trim();
            if texto.is_empty() {
                0.0
            } else {
                texto.parse::<f64>().ok()?
            }
        }
        Value::Bool(valor) => {
            if *valor {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        _ => return None,
    };
    if !numero.is_finite() || numero.fract() != 0.0 {
        return None;
    }
    if numero < i64::MIN as f64 || numero > i64::MAX as f64 {
        return None;
    }
    Some(numero as i64)
}

fn agrupar_desglose(
    desde: NaiveDate,
    dias: i64,
    tarifas: &[TarifaRango],
    precio_base: i64,
) -> Result<(Vec<Value>, i64), ApiError> {
    let mut desglose: Vec<Value> = Vec::new();
    let mut total: i64 = 0;
    for indice in 0..dias {
        let fecha = desde + Duration::days(indice);
        let clave = formatear(fecha);
        let tarifa = tarifas
            .iter()
            .find(|tarifa| tarifa.desde <= clave && clave <= tarifa.hasta);
        let precio = tarifa
            .map(|tarifa| tarifa.precio_cents)
            .unwrap_or(precio_base);
        let ultimo = desglose.last_mut();
        if let Some(Value::Object(objeto)) = ultimo {
            let mismo_precio = objeto
                .get("precio_cents")
                .and_then(Value::as_i64)
                .is_some_and(|anterior| anterior == precio);
            if mismo_precio {
                objeto.insert("hasta".into(), Value::String(clave));
                let dias_previos = objeto.get("dias").and_then(Value::as_i64).unwrap_or(0);
                objeto.insert("dias".into(), json!(dias_previos + 1));
                let subtotal_previo = objeto
                    .get("subtotal_cents")
                    .and_then(Value::as_i64)
                    .unwrap_or(0);
                let subtotal = subtotal_previo
                    .checked_add(precio)
                    .ok_or_else(total_invalido)?;
                objeto.insert("subtotal_cents".into(), json!(subtotal));
                total = total.checked_add(precio).ok_or_else(total_invalido)?;
                continue;
            }
        }
        desglose.push(json!({
            "desde": clave,
            "hasta": clave,
            "dias": 1,
            "precio_cents": precio,
            "subtotal_cents": precio,
        }));
        total = total.checked_add(precio).ok_or_else(total_invalido)?;
    }
    Ok((desglose, total))
}

fn total_invalido() -> ApiError {
    ApiError::dominio(
        "TOTAL_INVALIDO",
        "El total de la cotización excede el rango permitido.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fecha(valor: &str) -> NaiveDate {
        NaiveDate::parse_from_str(valor, "%Y-%m-%d").expect("fecha")
    }

    #[test]
    fn debe_parsear_solo_fechas_iso_validas() {
        assert_eq!(
            parse_fecha(&json!("2026-12-20")).expect("fecha"),
            fecha("2026-12-20")
        );
        assert!(parse_fecha(&json!("2026-02-30")).is_none());
        assert!(parse_fecha(&json!("2026-2-1")).is_none());
        assert!(parse_fecha(&json!("2026-12-20T00:00:00Z")).is_none());
        assert!(parse_fecha(&json!(20261220)).is_none());
    }

    #[test]
    fn debe_validar_el_rango_y_el_precio_de_una_tarifa() {
        let rango = validar_rango(&json!({
            "desde": "2026-12-20",
            "hasta": "2026-12-31",
            "precio_cents": 25_000,
        }))
        .expect("rango");
        assert_eq!(rango.desde, "2026-12-20");
        assert_eq!(rango.hasta, "2026-12-31");
        assert_eq!(rango.precio, 25_000);

        assert_eq!(
            validar_rango(&json!({
                "desde": "2026-12-31",
                "hasta": "2026-12-20",
                "precio_cents": 1,
            }))
            .expect_err("invertido")
            .codigo,
            "RANGO_INVALIDO"
        );
        assert_eq!(
            validar_rango(&json!({
                "desde": "2026-12-20",
                "hasta": "2026-12-21",
                "precio_cents": -1,
            }))
            .expect_err("precio")
            .codigo,
            "PRECIO_INVALIDO"
        );
    }

    #[test]
    fn debe_agrupar_dias_consecutivos_por_precio() {
        let tarifas = vec![
            TarifaRango {
                desde: "2026-12-21".to_string(),
                hasta: "2026-12-22".to_string(),
                precio_cents: 30_000,
            },
            TarifaRango {
                desde: "2026-12-24".to_string(),
                hasta: "2026-12-24".to_string(),
                precio_cents: 50_000,
            },
        ];
        let (desglose, total) =
            agrupar_desglose(fecha("2026-12-20"), 6, &tarifas, 20_000).expect("desglose");
        assert_eq!(total, 20_000 + 30_000 + 30_000 + 20_000 + 50_000 + 20_000);
        assert_eq!(desglose.len(), 5);
        assert_eq!(
            desglose[1],
            json!({
                "desde": "2026-12-21",
                "hasta": "2026-12-22",
                "dias": 2,
                "precio_cents": 30_000,
                "subtotal_cents": 60_000,
            })
        );
    }

    #[test]
    fn debe_calcular_los_dias_del_rango_de_forma_inclusiva() {
        let desde = fecha("2026-12-20");
        let hasta = fecha("2026-12-22");
        assert_eq!((hasta - desde).num_days() + 1, 3);
        assert!((fecha("2026-12-20") - fecha("2026-12-22")).num_days() + 1 < 1);
    }
}
