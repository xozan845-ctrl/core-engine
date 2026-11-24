//! Inventario: configuración, niveles de stock, proveedores y lotes.
//!
//! Réplica de `inventario.service.ts` (niveles H-06, proveedores y lotes con
//! consumo FEFO) y de la configuración de inventario de
//! `operaciones.service.ts`. Los montos se serializan como `"1150.00"` y las
//! fechas `date` como medianoche local en ISO 8601 con milisegundos, igual que
//! `node-postgres`.

use crate::db::{fila_a_json, interno, Db};
use crate::errors::ApiError;
use crate::outbox::Outbox;
use chrono::{Local, NaiveDate, TimeZone};
use serde_json::{json, Map, Value};
use sqlx::postgres::PgRow;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

/// Métodos de costeo admitidos por `inventory.configuracion_inventario`.
pub const METODOS_COSTEO: [&str; 3] = ["promedio", "fifo", "ueps"];

pub struct Inventario {
    db: Arc<Db>,
    outbox: Option<Arc<Outbox>>,
}

/// Lote candidato para el consumo FEFO (ya ordenado por la consulta).
#[derive(Debug, Clone, PartialEq, Eq)]
struct LoteDisponible {
    id: String,
    codigo_lote: String,
    cantidad_disponible: i64,
    vencido: bool,
}

/// Consumo calculado para un lote.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Consumo {
    lote_id: String,
    codigo_lote: String,
    cantidad: i64,
}

fn texto(datos: &Value, clave: &str) -> String {
    datos
        .get(clave)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

/// `Number.isInteger(valor)`: acepta enteros y decimales exactos (`1.0`).
fn entero(valor: &Value) -> Option<i64> {
    valor.as_i64().or_else(|| {
        valor
            .as_f64()
            .filter(|numero| numero.is_finite() && numero.fract() == 0.0)
            .and_then(|numero| {
                if numero >= i64::MIN as f64 && numero <= i64::MAX as f64 {
                    Some(numero as i64)
                } else {
                    None
                }
            })
    })
}

/// `date` de PostgreSQL: `node-postgres` la entrega como medianoche local y
/// `JSON.stringify` la convierte a ISO UTC con milisegundos.
fn fecha_iso(fecha: NaiveDate) -> String {
    match fecha.and_hms_opt(0, 0, 0).and_then(|medianoche| {
        Local
            .from_local_datetime(&medianoche)
            .single()
            .map(|valor| valor.with_timezone(&chrono::Utc))
    }) {
        Some(valor) => crate::db::iso_ms(valor),
        None => format!("{}T00:00:00.000Z", fecha.format("%Y-%m-%d")),
    }
}

fn lote_a_json(fila: &PgRow) -> Result<Value, ApiError> {
    let mut valor = fila_a_json(fila).map_err(interno)?;
    let fecha: Option<NaiveDate> = fila.try_get("vence_en").map_err(interno)?;
    if let Some(objeto) = valor.as_object_mut() {
        objeto.insert(
            "vence_en".into(),
            fecha
                .map(|fecha| Value::String(fecha_iso(fecha)))
                .unwrap_or(Value::Null),
        );
    }
    Ok(valor)
}

/// Reparte `cantidad` entre los lotes vigentes en el orden recibido (FEFO).
/// Devuelve la cantidad que falta cuando el total no alcanza.
fn plan_consumo(lotes: &[LoteDisponible], cantidad: i64) -> Result<Vec<Consumo>, i64> {
    let mut restante = cantidad;
    let mut plan = Vec::new();
    for lote in lotes.iter().filter(|lote| !lote.vencido) {
        if restante == 0 {
            break;
        }
        let consumir = restante.min(lote.cantidad_disponible);
        plan.push(Consumo {
            lote_id: lote.id.clone(),
            codigo_lote: lote.codigo_lote.clone(),
            cantidad: consumir,
        });
        restante -= consumir;
    }
    if restante > 0 {
        Err(restante)
    } else {
        Ok(plan)
    }
}

fn booleano_configuracion(cambios: &Value, clave: &str, actual: bool) -> bool {
    match cambios.get(clave) {
        None | Some(Value::Null) => actual,
        Some(valor) => valor.as_bool().unwrap_or(actual),
    }
}

mod configuracion;
mod consultas;
mod escritura;

impl Inventario {
    pub fn nueva(db: Arc<Db>, outbox: Option<Arc<Outbox>>) -> Self {
        Self { db, outbox }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lote(id: &str, disponible: i64, vencido: bool) -> LoteDisponible {
        LoteDisponible {
            id: id.to_string(),
            codigo_lote: id.to_uppercase(),
            cantidad_disponible: disponible,
            vencido,
        }
    }

    #[test]
    fn debe_repartir_el_consumo_en_orden_fefo() {
        let lotes = vec![lote("l1", 5, false), lote("l2", 10, false)];
        let plan = plan_consumo(&lotes, 8).expect("plan");
        assert_eq!(
            plan,
            vec![
                Consumo {
                    lote_id: "l1".into(),
                    codigo_lote: "L1".into(),
                    cantidad: 5
                },
                Consumo {
                    lote_id: "l2".into(),
                    codigo_lote: "L2".into(),
                    cantidad: 3
                },
            ]
        );
    }

    #[test]
    fn debe_omitir_lotes_vencidos_y_reportar_el_faltante() {
        let lotes = vec![lote("l1", 5, true), lote("l2", 2, false)];
        let plan = plan_consumo(&lotes, 2).expect("plan");
        assert_eq!(plan.len(), 1);
        assert_eq!(plan[0].cantidad, 2);
        assert_eq!(plan_consumo(&lotes, 5).expect_err("faltante"), 3);
        assert!(plan_consumo(&lotes, 2).is_ok());
    }

    #[test]
    fn debe_aceptar_enteros_exactos_y_rechazar_fracciones() {
        assert_eq!(entero(&json!(10)), Some(10));
        assert_eq!(entero(&json!(10.0)), Some(10));
        assert_eq!(entero(&json!(10.5)), None);
        assert_eq!(entero(&json!("10")), None);
        assert_eq!(entero(&Value::Null), None);
    }

    #[test]
    fn debe_serializar_fechas_como_medianoche_local() {
        let fecha = NaiveDate::from_ymd_opt(2027, 1, 31).expect("fecha");
        let iso = fecha_iso(fecha);
        assert!(iso.ends_with(".000Z"));
        let parseada = chrono::DateTime::parse_from_rfc3339(&iso).expect("iso");
        assert_eq!(parseada.with_timezone(&Local).date_naive(), fecha);
    }

    #[test]
    fn debe_validar_los_metodos_de_costeo() {
        assert!(METODOS_COSTEO.contains(&"promedio"));
        assert!(METODOS_COSTEO.contains(&"fifo"));
        assert!(METODOS_COSTEO.contains(&"ueps"));
        assert!(!METODOS_COSTEO.contains(&"lifo"));
    }
}
