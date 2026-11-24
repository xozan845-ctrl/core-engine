//! Régimen fiscal (cap. 4.4) multijurisdicción: cada país es una fila en
//! `finance.jurisdicciones` (leyes, tasas y periodicidad configurables).
//! Nicaragua: Ley 822 (IVA 15 %, IR) y declaraciones mensuales.

mod configuracion;
mod declaraciones;
mod jurisdicciones;
mod regimenes;

mod prelude {
    pub(crate) use crate::contabilidad::Contabilidad;
    pub(crate) use crate::db::{iso_ms, jsonb, timestamp, uuid_texto, Db};
    pub(crate) use crate::errors::{error_db, es_violacion_unicidad, ApiError};
    pub(crate) use crate::outbox::{Outbox, DECLARACION_GENERADA};
    pub(crate) use chrono::{DateTime, Datelike, Local, NaiveDate, Utc};
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::{json, Value};
    pub(crate) use sqlx::Row;
    pub(crate) use std::sync::Arc;
}
use self::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Jurisdiccion {
    pub codigo_pais: String,
    pub nombre: String,
    /// País ISO 3166-1 alpha-2 (p. ej. `NI`).
    pub pais_iso: String,
    /// Subdivisión opcional para regiones subnacionales (p. ej. `CA`).
    pub subdivision: Option<String>,
    /// Símbolo histórico que usan los comprobantes (p. ej. `C$`).
    pub moneda: String,
    /// Código ISO 4217 de operación (p. ej. `NIO`).
    pub moneda_iso: String,
    pub simbolo_moneda: String,
    pub locale: String,
    pub zona_horaria: String,
    pub tasa_iva: f64,
    pub tasa_ir: f64,
    pub periodicidad_declaracion: String,
    pub documento_fiscal: Value,
    pub perfil: Value,
    pub leyes: Value,
    pub vigente_desde: Option<String>,
    pub tasa_default_codigo: String,
    pub retencion_activa: bool,
    pub tasa_retencion: f64,
    pub retencion_codigo: String,
    pub activa: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TasaImpuesto {
    pub codigo: String,
    pub nombre: String,
    pub tasa: f64,
    pub exenta: bool,
    pub activa: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfiguracionFacturacion {
    pub jurisdiccion: Jurisdiccion,
    pub tasas: Vec<TasaImpuesto>,
    pub facturacion_electronica: bool,
    pub formato_pdf: bool,
    pub proveedor_electronico: String,
    pub ambiente_electronico: String,
    pub impuesto_activo: bool,
    pub precios_con_impuesto: bool,
    pub retencion_activa: bool,
    pub tasa_retencion: f64,
    pub retencion_codigo: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegimenFiscal {
    pub id: String,
    pub jurisdiccion: String,
    pub codigo: String,
    pub nombre: String,
    pub descripcion: String,
    pub periodicidad: String,
    pub condicion_ingresos_anuales_cents: Option<i64>,
    pub estado: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SujetoTributario {
    pub id: String,
    pub jurisdiccion: String,
    pub regimen_id: Option<String>,
    pub regimen_nombre: Option<String>,
    pub razon_social: String,
    pub ruc: Option<String>,
    pub es_plataforma: bool,
    pub estado: String,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Declaracion {
    pub id: String,
    pub empresa_id: Option<String>,
    pub jurisdiccion: String,
    pub tipo: String,
    pub periodo_inicio: String,
    pub periodo_fin: String,
    pub base_gravada_cents: i64,
    pub iva_debitado_cents: i64,
    pub iva_credito_cents: i64,
    pub iva_a_pagar_cents: i64,
    pub renta_bruta_cents: i64,
    pub renta_gravable_cents: i64,
    pub ir_a_pagar_cents: i64,
    pub cuota_cents: i64,
    pub estado: String,
    pub detalle: Value,
    pub generada_en: Option<String>,
    pub presentada_en: Option<String>,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PeriodoMensual {
    pub inicio: String,
    pub fin: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearJurisdiccion {
    pub codigo_pais: String,
    pub nombre: String,
    pub moneda: String,
    pub simbolo_moneda: String,
    pub tasa_iva_por_mil: i64,
    pub tasa_ir_por_mil: i64,
    #[serde(default)]
    pub periodicidad_declaracion: Option<String>,
    #[serde(default)]
    pub leyes: Option<Value>,
    #[serde(default)]
    pub pais_iso: Option<String>,
    #[serde(default)]
    pub subdivision: Option<String>,
    #[serde(default)]
    pub moneda_iso: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub zona_horaria: Option<String>,
    #[serde(default)]
    pub documento_fiscal: Option<Value>,
    #[serde(default)]
    pub perfil: Option<Value>,
    #[serde(default)]
    pub tasa_default_codigo: Option<String>,
    #[serde(default)]
    pub retencion_activa: Option<bool>,
    #[serde(default)]
    pub tasa_retencion_por_mil: Option<i64>,
    #[serde(default)]
    pub retencion_codigo: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ActualizarJurisdiccion {
    #[serde(default)]
    pub nombre: Option<String>,
    #[serde(default)]
    pub activa: Option<bool>,
    #[serde(default)]
    pub tasa_iva_por_mil: Option<i64>,
    #[serde(default)]
    pub tasa_ir_por_mil: Option<i64>,
    #[serde(default)]
    pub tasa_default_codigo: Option<String>,
    #[serde(default)]
    pub retencion_activa: Option<bool>,
    #[serde(default)]
    pub tasa_retencion_por_mil: Option<i64>,
    #[serde(default)]
    pub retencion_codigo: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub zona_horaria: Option<String>,
    #[serde(default)]
    pub documento_fiscal: Option<Value>,
    #[serde(default)]
    pub perfil: Option<Value>,
    #[serde(default)]
    pub leyes: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearTasaImpuesto {
    pub codigo: String,
    pub nombre: String,
    pub tasa_por_mil: i64,
    #[serde(default)]
    pub exenta: bool,
    #[serde(default)]
    pub activa: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearRegimen {
    pub jurisdiccion: String,
    pub codigo: String,
    pub nombre: String,
    #[serde(default)]
    pub descripcion: Option<String>,
    #[serde(default)]
    pub periodicidad: Option<String>,
    #[serde(default)]
    pub condicion_ingresos_anuales_cents: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RegistrarSujeto {
    pub id: String,
    #[serde(default)]
    pub jurisdiccion: Option<String>,
    #[serde(default)]
    pub regimen_id: Option<String>,
    pub razon_social: String,
    #[serde(default)]
    pub ruc: Option<String>,
    #[serde(default)]
    pub es_plataforma: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct FiltrosDeclaracion {
    #[serde(default)]
    pub empresa_id: Option<String>,
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub periodo_inicio: Option<String>,
    #[serde(default)]
    pub estado: Option<String>,
}

/// Columnas del descriptor de región. Se comparten entre las consultas para
/// que añadir un campo no obligue a tocar cada `SELECT`.
const COLUMNAS_JURISDICCION: &str = "codigo_pais, nombre, pais_iso, subdivision, moneda,
    moneda_iso, simbolo_moneda, locale, zona_horaria, tasa_iva, tasa_ir,
    periodicidad_declaracion, documento_fiscal, perfil, leyes, vigente_desde,
    tasa_default_codigo, retencion_activa, tasa_retencion, retencion_codigo, activa";

fn columnas_jurisdiccion(alias: &str) -> String {
    COLUMNAS_JURISDICCION
        .split(',')
        .map(|columna| format!("{}.{}", alias, columna.trim()))
        .collect::<Vec<_>>()
        .join(", ")
}

pub struct Tributacion {
    db: Arc<Db>,
    outbox: Outbox,
    contabilidad: Arc<Contabilidad>,
}

impl Tributacion {
    pub fn nueva(db: Arc<Db>, outbox: Outbox, contabilidad: Arc<Contabilidad>) -> Self {
        Self {
            db,
            outbox,
            contabilidad,
        }
    }

    // ── jurisdicciones ─────────────────────────────────────────────────────
}

fn campo(datos: &Value, clave: &str) -> i32 {
    datos
        .get(clave)
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

fn fecha_utc(fecha: &str) -> Result<DateTime<Utc>, ApiError> {
    let naive = NaiveDate::parse_from_str(fecha, "%Y-%m-%d")
        .map_err(|_| ApiError::dominio("FECHA_INVALIDA", "La fecha debe ser YYYY-MM-DD."))?;
    Ok(DateTime::<Utc>::from_naive_utc_and_offset(
        naive.and_hms_opt(0, 0, 0).unwrap_or_default(),
        Utc,
    ))
}

fn jurisdiccion_de_fila(fila: &sqlx::postgres::PgRow) -> Jurisdiccion {
    let vigente_desde: Option<NaiveDate> = fila.try_get("vigente_desde").unwrap_or(None);
    Jurisdiccion {
        codigo_pais: fila.try_get("codigo_pais").unwrap_or_default(),
        nombre: fila.try_get("nombre").unwrap_or_default(),
        pais_iso: fila
            .try_get::<Option<String>, _>("pais_iso")
            .unwrap_or(None)
            .unwrap_or_default(),
        subdivision: fila.try_get("subdivision").unwrap_or(None),
        moneda: fila.try_get("moneda").unwrap_or_default(),
        moneda_iso: fila
            .try_get::<Option<String>, _>("moneda_iso")
            .unwrap_or(None)
            .unwrap_or_default(),
        simbolo_moneda: fila.try_get("simbolo_moneda").unwrap_or_default(),
        locale: fila.try_get("locale").unwrap_or_else(|_| "es".into()),
        zona_horaria: fila
            .try_get("zona_horaria")
            .unwrap_or_else(|_| "UTC".into()),
        tasa_iva: fila.try_get::<f64, _>("tasa_iva").unwrap_or(0.0),
        tasa_ir: fila.try_get::<f64, _>("tasa_ir").unwrap_or(0.0),
        periodicidad_declaracion: fila.try_get("periodicidad_declaracion").unwrap_or_default(),
        documento_fiscal: jsonb(fila, "documento_fiscal").unwrap_or_else(|_| json!({})),
        perfil: jsonb(fila, "perfil").unwrap_or_else(|_| json!({})),
        leyes: jsonb(fila, "leyes").unwrap_or_else(|_| json!({})),
        vigente_desde: vigente_desde.map(|fecha| fecha.to_string()),
        tasa_default_codigo: fila
            .try_get("tasa_default_codigo")
            .unwrap_or_else(|_| "GENERAL".to_string()),
        retencion_activa: fila.try_get("retencion_activa").unwrap_or(false),
        tasa_retencion: fila.try_get::<f64, _>("tasa_retencion").unwrap_or(0.0),
        retencion_codigo: fila
            .try_get("retencion_codigo")
            .unwrap_or_else(|_| "IR".to_string()),
        activa: fila.try_get("activa").unwrap_or(false),
    }
}

fn tasa_de_fila(fila: &sqlx::postgres::PgRow) -> TasaImpuesto {
    TasaImpuesto {
        codigo: fila.try_get("codigo").unwrap_or_default(),
        nombre: fila.try_get("nombre").unwrap_or_default(),
        tasa: fila.try_get::<f64, _>("tasa").unwrap_or(0.0),
        exenta: fila.try_get("exenta").unwrap_or(false),
        activa: fila.try_get("activa").unwrap_or(false),
    }
}

fn regimen_de_fila(fila: &sqlx::postgres::PgRow) -> RegimenFiscal {
    RegimenFiscal {
        id: uuid_texto(fila, "id").unwrap_or_default(),
        jurisdiccion: fila.try_get("jurisdiccion").unwrap_or_default(),
        codigo: fila.try_get("codigo").unwrap_or_default(),
        nombre: fila.try_get("nombre").unwrap_or_default(),
        descripcion: fila.try_get("descripcion").unwrap_or_default(),
        periodicidad: fila.try_get("periodicidad").unwrap_or_default(),
        condicion_ingresos_anuales_cents: fila
            .try_get::<Option<i32>, _>("condicion_ingresos_anuales_cents")
            .ok()
            .flatten()
            .map(|valor| valor as i64),
        estado: fila.try_get("estado").unwrap_or_default(),
    }
}

fn sujeto_de_fila(fila: &sqlx::postgres::PgRow) -> SujetoTributario {
    SujetoTributario {
        id: uuid_texto(fila, "id").unwrap_or_default(),
        jurisdiccion: fila.try_get("jurisdiccion").unwrap_or_default(),
        regimen_id: uuid_texto(fila, "regimen_id").ok(),
        regimen_nombre: fila.try_get("regimen_nombre").unwrap_or(None),
        razon_social: fila.try_get("razon_social").unwrap_or_default(),
        ruc: fila.try_get("ruc").unwrap_or(None),
        es_plataforma: fila.try_get("es_plataforma").unwrap_or(false),
        estado: fila.try_get("estado").unwrap_or_default(),
        creado_en: timestamp(fila, "creado_en").unwrap_or_default(),
    }
}

fn declaracion_de_fila(fila: &sqlx::postgres::PgRow) -> Declaracion {
    Declaracion {
        id: uuid_texto(fila, "id").unwrap_or_default(),
        empresa_id: uuid_texto(fila, "empresa_id").ok(),
        jurisdiccion: fila.try_get("jurisdiccion").unwrap_or_default(),
        tipo: fila.try_get("tipo").unwrap_or_default(),
        periodo_inicio: fila
            .try_get::<NaiveDate, _>("periodo_inicio")
            .map(|fecha| fecha.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        periodo_fin: fila
            .try_get::<NaiveDate, _>("periodo_fin")
            .map(|fecha| fecha.format("%Y-%m-%d").to_string())
            .unwrap_or_default(),
        base_gravada_cents: fila.try_get::<i32, _>("base_gravada_cents").unwrap_or(0) as i64,
        iva_debitado_cents: fila.try_get::<i32, _>("iva_debitado_cents").unwrap_or(0) as i64,
        iva_credito_cents: fila.try_get::<i32, _>("iva_credito_cents").unwrap_or(0) as i64,
        iva_a_pagar_cents: fila.try_get::<i32, _>("iva_a_pagar_cents").unwrap_or(0) as i64,
        renta_bruta_cents: fila.try_get::<i32, _>("renta_bruta_cents").unwrap_or(0) as i64,
        renta_gravable_cents: fila.try_get::<i32, _>("renta_gravable_cents").unwrap_or(0) as i64,
        ir_a_pagar_cents: fila.try_get::<i32, _>("ir_a_pagar_cents").unwrap_or(0) as i64,
        cuota_cents: fila.try_get::<i32, _>("cuota_cents").unwrap_or(0) as i64,
        estado: fila.try_get("estado").unwrap_or_default(),
        detalle: jsonb(fila, "detalle").unwrap_or_else(|_| Value::Null),
        generada_en: timestamp_opt(fila, "generada_en"),
        presentada_en: timestamp_opt(fila, "presentada_en"),
        creado_en: timestamp(fila, "creado_en").unwrap_or_default(),
    }
}

fn timestamp_opt(fila: &sqlx::postgres::PgRow, columna: &str) -> Option<String> {
    fila.try_get::<Option<DateTime<Utc>>, _>(columna)
        .ok()
        .flatten()
        .map(iso_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fecha(anio: i32, mes: u32, dia: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(anio, mes, dia).expect("fecha")
    }

    #[test]
    fn debe_calcular_el_periodo_mensual() {
        let periodo = Tributacion::periodo_del_mes(fecha(2026, 11, 13));
        assert_eq!(periodo.inicio, "2026-11-01");
        assert_eq!(periodo.fin, "2026-11-30");
        let febrero = Tributacion::periodo_del_mes(fecha(2024, 2, 10));
        assert_eq!(febrero.fin, "2024-02-29");
    }

    #[test]
    fn debe_calcular_el_mes_anterior_incluso_en_enero() {
        assert_eq!(
            Tributacion::mes_anterior(fecha(2026, 11, 13)),
            PeriodoMensual {
                inicio: "2026-10-01".into(),
                fin: "2026-10-31".into()
            }
        );
        assert_eq!(
            Tributacion::mes_anterior(fecha(2026, 1, 5)),
            PeriodoMensual {
                inicio: "2025-12-01".into(),
                fin: "2025-12-31".into()
            }
        );
    }

    #[test]
    fn debe_leer_los_campos_del_plan() {
        let valor = json!({"iva_a_pagar_cents": 1500});
        assert_eq!(campo(&valor, "iva_a_pagar_cents"), 1500);
        assert_eq!(campo(&valor, "otro"), 0);
    }
}
