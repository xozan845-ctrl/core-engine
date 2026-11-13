//! Régimen fiscal (cap. 4.4) multijurisdicción: cada país es una fila en
//! `finance.jurisdicciones` (leyes, tasas y periodicidad configurables).
//! Nicaragua: Ley 822 (IVA 15 %, IR) y declaraciones mensuales.

use crate::contabilidad::Contabilidad;
use crate::db::{iso_ms, jsonb, timestamp, uuid_texto, Db};
use crate::errors::{error_db, es_violacion_unicidad, ApiError};
use crate::outbox::{Outbox, DECLARACION_GENERADA};
use chrono::{DateTime, Datelike, Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::Row;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Jurisdiccion {
    pub codigo_pais: String,
    pub nombre: String,
    pub moneda: String,
    pub simbolo_moneda: String,
    pub tasa_iva: f64,
    pub tasa_ir: f64,
    pub periodicidad_declaracion: String,
    pub leyes: Value,
    pub activa: bool,
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
    pub tipo: Option<String>,
    #[serde(default)]
    pub periodo_inicio: Option<String>,
    #[serde(default)]
    pub estado: Option<String>,
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

    pub async fn jurisdiccion_activa(&self) -> Result<Option<Jurisdiccion>, ApiError> {
        let fila = sqlx::query(
            "SELECT codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir,
                    periodicidad_declaracion, leyes, activa
             FROM finance.jurisdicciones WHERE activa ORDER BY codigo_pais LIMIT 1",
        )
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(jurisdiccion_de_fila))
    }

    pub async fn listar_jurisdicciones(&self) -> Result<Vec<Jurisdiccion>, ApiError> {
        let filas = sqlx::query(
            "SELECT codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir,
                    periodicidad_declaracion, leyes, activa
             FROM finance.jurisdicciones ORDER BY codigo_pais",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(jurisdiccion_de_fila).collect())
    }

    pub async fn crear_jurisdiccion(
        &self,
        datos: CrearJurisdiccion,
    ) -> Result<Jurisdiccion, ApiError> {
        if datos.codigo_pais.trim().len() < 2 {
            return Err(ApiError::validacion(json!([
                {"campo": "codigo_pais", "restricciones": ["minLength:2"]}
            ])));
        }
        if datos.tasa_iva_por_mil < 0 || datos.tasa_ir_por_mil < 0 {
            return Err(ApiError::validacion(json!([
                {"campo": "tasa_iva_por_mil", "restricciones": ["min:0"]},
                {"campo": "tasa_ir_por_mil", "restricciones": ["min:0"]}
            ])));
        }
        let fila = sqlx::query(
            "INSERT INTO finance.jurisdicciones
               (codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir,
                periodicidad_declaracion, leyes)
             VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, 'MENSUAL'), COALESCE($8::jsonb, '{}'))
             RETURNING codigo_pais, nombre, moneda, simbolo_moneda, tasa_iva, tasa_ir,
                       periodicidad_declaracion, leyes, activa",
        )
        .bind(datos.codigo_pais.to_uppercase())
        .bind(datos.nombre)
        .bind(datos.moneda)
        .bind(datos.simbolo_moneda)
        .bind(datos.tasa_iva_por_mil as f64 / 1000.0)
        .bind(datos.tasa_ir_por_mil as f64 / 1000.0)
        .bind(datos.periodicidad_declaracion)
        .bind(datos.leyes.map(sqlx::types::Json))
        .fetch_optional(self.db.pool())
        .await
        .map_err(|error| {
            if es_violacion_unicidad(&error) {
                ApiError::conflicto("La jurisdiccion ya existe.")
            } else {
                error_db(error)
            }
        })?
        .ok_or_else(|| ApiError::conflicto("La jurisdiccion ya existe."))?;
        Ok(jurisdiccion_de_fila(&fila))
    }

    // ── regímenes ──────────────────────────────────────────────────────────

    pub async fn regimenes_de(
        &self,
        jurisdiccion: Option<&str>,
    ) -> Result<Vec<RegimenFiscal>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, jurisdiccion, codigo, nombre, descripcion, periodicidad,
                    condicion_ingresos_anuales_cents, estado
             FROM finance.regimenes_fiscales
             WHERE ($1::text IS NULL OR jurisdiccion = $1)
             ORDER BY jurisdiccion, codigo",
        )
        .bind(jurisdiccion)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(regimen_de_fila).collect())
    }

    pub async fn crear_regimen(
        &self,
        datos: CrearRegimen,
    ) -> Result<Option<RegimenFiscal>, ApiError> {
        if datos.jurisdiccion.trim().len() < 2 {
            return Err(ApiError::validacion(json!([
                {"campo": "jurisdiccion", "restricciones": ["minLength:2"]}
            ])));
        }
        let fila = sqlx::query(
            "INSERT INTO finance.regimenes_fiscales
               (jurisdiccion, codigo, nombre, descripcion, periodicidad,
                condicion_ingresos_anuales_cents)
             VALUES ($1, $2, $3, COALESCE($4, ''), COALESCE($5, 'MENSUAL'), $6)
             RETURNING id, jurisdiccion, codigo, nombre, descripcion, periodicidad,
                       condicion_ingresos_anuales_cents, estado",
        )
        .bind(datos.jurisdiccion.to_uppercase())
        .bind(datos.codigo)
        .bind(datos.nombre)
        .bind(datos.descripcion)
        .bind(datos.periodicidad)
        .bind(
            datos
                .condicion_ingresos_anuales_cents
                .map(|valor| valor as i32),
        )
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(regimen_de_fila))
    }

    // ── sujetos fiscales ───────────────────────────────────────────────────

    pub async fn sujetos(&self) -> Result<Vec<SujetoTributario>, ApiError> {
        let filas = sqlx::query(
            "SELECT s.id, s.jurisdiccion, s.regimen_id, r.nombre AS regimen_nombre,
                    s.razon_social, s.ruc, s.es_plataforma, s.estado, s.creado_en
             FROM finance.sujetos_fiscales s
             LEFT JOIN finance.regimenes_fiscales r ON r.id = s.regimen_id
             ORDER BY s.creado_en DESC LIMIT 300",
        )
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(sujeto_de_fila).collect())
    }

    pub async fn sujeto_fiscal_de(
        &self,
        usuario_id: &str,
    ) -> Result<Option<SujetoTributario>, ApiError> {
        let uuid = uuid::Uuid::parse_str(usuario_id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El usuario debe ser un UUID."))?;
        let fila = sqlx::query(
            "SELECT s.id, s.jurisdiccion, s.regimen_id, r.nombre AS regimen_nombre,
                    s.razon_social, s.ruc, s.es_plataforma, s.estado, s.creado_en
             FROM finance.sujetos_fiscales s
             LEFT JOIN finance.regimenes_fiscales r ON r.id = s.regimen_id
             WHERE s.id = $1",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(sujeto_de_fila))
    }

    pub async fn registrar_sujeto(
        &self,
        datos: RegistrarSujeto,
    ) -> Result<Option<SujetoTributario>, ApiError> {
        let uuid = uuid::Uuid::parse_str(&datos.id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El sujeto debe ser un UUID."))?;
        if datos.razon_social.trim().is_empty() {
            return Err(ApiError::validacion(json!([
                {"campo": "razon_social", "restricciones": ["isNotEmpty"]}
            ])));
        }
        let jurisdiccion = match datos.jurisdiccion {
            Some(jurisdiccion) => jurisdiccion,
            None => self
                .jurisdiccion_activa()
                .await?
                .map(|jurisdiccion| jurisdiccion.codigo_pais)
                .ok_or_else(|| {
                    ApiError::dominio(
                        "SIN_JURISDICCION",
                        "No hay jurisdiccion activa configurada.",
                    )
                })?,
        };
        let regimen =
            match datos.regimen_id {
                Some(regimen) => Some(uuid::Uuid::parse_str(&regimen).map_err(|_| {
                    ApiError::dominio("ID_INVALIDO", "El regimen debe ser un UUID.")
                })?),
                None => None,
            };
        let fila = sqlx::query(
            "INSERT INTO finance.sujetos_fiscales
               (id, jurisdiccion, regimen_id, razon_social, ruc, es_plataforma)
             VALUES ($1, $2, $3, $4, $5, COALESCE($6, FALSE))
             RETURNING id, jurisdiccion, regimen_id, razon_social, ruc, es_plataforma,
                       estado, creado_en",
        )
        .bind(uuid)
        .bind(jurisdiccion)
        .bind(regimen)
        .bind(datos.razon_social)
        .bind(datos.ruc)
        .bind(datos.es_plataforma.unwrap_or(false))
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(sujeto_de_fila))
    }

    pub async fn dar_de_baja(
        &self,
        usuario_id: &str,
    ) -> Result<Option<SujetoTributario>, ApiError> {
        let uuid = uuid::Uuid::parse_str(usuario_id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "El usuario debe ser un UUID."))?;
        let fila = sqlx::query(
            "UPDATE finance.sujetos_fiscales SET estado = 'BAJA'
             WHERE id = $1 AND estado = 'ACTIVO'
             RETURNING id, jurisdiccion, regimen_id, razon_social, ruc, es_plataforma,
                       estado, creado_en",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(fila.as_ref().map(sujeto_de_fila))
    }

    // ── declaraciones ──────────────────────────────────────────────────────

    /// Periodo mensual (inicio día 1, fin último día) de una fecha.
    pub fn periodo_del_mes(fecha: NaiveDate) -> PeriodoMensual {
        let inicio = NaiveDate::from_ymd_opt(fecha.year(), fecha.month(), 1).unwrap_or(fecha);
        let fin = if fecha.month() == 12 {
            NaiveDate::from_ymd_opt(fecha.year() + 1, 1, 1)
        } else {
            NaiveDate::from_ymd_opt(fecha.year(), fecha.month() + 1, 1)
        }
        .and_then(|siguiente| siguiente.pred_opt())
        .unwrap_or(fecha);
        PeriodoMensual {
            inicio: inicio.format("%Y-%m-%d").to_string(),
            fin: fin.format("%Y-%m-%d").to_string(),
        }
    }

    /// Mes anterior al recibido: es el corte que genera la declaración.
    pub fn mes_anterior(fecha: NaiveDate) -> PeriodoMensual {
        let primero = NaiveDate::from_ymd_opt(fecha.year(), fecha.month(), 1).unwrap_or(fecha);
        let previo = primero.pred_opt().unwrap_or(fecha);
        Self::periodo_del_mes(previo)
    }

    pub async fn declaraciones(
        &self,
        filtros: FiltrosDeclaracion,
    ) -> Result<Vec<Declaracion>, ApiError> {
        let filas = sqlx::query(
            "SELECT id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                    base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                    iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                    ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                    presentada_en, creado_en
             FROM finance.declaraciones
             WHERE ($1::text IS NULL OR tipo = $1)
               AND ($2::date IS NULL OR periodo_inicio = $2::date)
               AND ($3::text IS NULL OR estado = $3)
             ORDER BY periodo_inicio DESC, tipo LIMIT 300",
        )
        .bind(filtros.tipo.as_deref())
        .bind(filtros.periodo_inicio.as_deref())
        .bind(filtros.estado.as_deref())
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;
        Ok(filas.iter().map(declaracion_de_fila).collect())
    }

    pub async fn generar_declaraciones_del_mes_anterior(
        &self,
        fecha: NaiveDate,
    ) -> Result<Vec<Declaracion>, ApiError> {
        self.generar_declaraciones(Some(Self::mes_anterior(fecha)))
            .await
    }

    /// Genera IVA e IR (más CUOTA_FIJA informativa) del periodo. Idempotente
    /// por `(jurisdiccion, tipo, periodo_inicio)`.
    pub async fn generar_declaraciones(
        &self,
        periodo: Option<PeriodoMensual>,
    ) -> Result<Vec<Declaracion>, ApiError> {
        let jurisdiccion = self.jurisdiccion_activa().await?.ok_or_else(|| {
            ApiError::dominio(
                "SIN_JURISDICCION",
                "No hay jurisdiccion activa configurada.",
            )
        })?;
        let mes = periodo.unwrap_or_else(|| Self::mes_anterior(Local::now().date_naive()));
        let inicio = fecha_utc(&mes.inicio)?;
        let fin = fecha_utc(&mes.fin)?;
        let ingresos = self.contabilidad.ingresos_del_periodo(inicio, fin).await?;
        let gastos = self.contabilidad.gastos_del_periodo(inicio, fin).await?;

        let iva = (ingresos as f64 * jurisdiccion.tasa_iva).round() as i64;
        let ir_base = (ingresos - gastos).max(0);
        let ir = (ir_base as f64 * jurisdiccion.tasa_ir).round() as i64;

        let plan = vec![
            (
                "IVA",
                json!({
                    "base_gravada_cents": ingresos,
                    "iva_debitado_cents": iva,
                    "iva_credito_cents": 0,
                    "iva_a_pagar_cents": iva,
                }),
                json!({
                    "tasa": jurisdiccion.tasa_iva,
                    "ley": jurisdiccion
                        .leyes
                        .get("iva")
                        .and_then(Value::as_str)
                        .unwrap_or("Ley 822"),
                }),
            ),
            (
                "IR",
                json!({
                    "renta_bruta_cents": ingresos,
                    "renta_gravable_cents": ir_base,
                    "ir_a_pagar_cents": ir,
                }),
                json!({
                    "tasa": jurisdiccion.tasa_ir,
                    "gastos_del_periodo_cents": gastos,
                    "ley": "Ley 822 (IR)",
                }),
            ),
            (
                "CUOTA_FIJA",
                json!({"cuota_cents": 0}),
                json!({
                    "informativo": true,
                    "nota": "Cuota determinada por la DGI para cada contribuyente; la plataforma documenta comisiones para su declaracion.",
                }),
            ),
        ];

        let mut generadas = Vec::new();
        for (tipo, campos, detalle) in plan {
            if tipo != "IVA" && tipo != "IR" && ingresos == 0 && gastos == 0 {
                continue;
            }
            let existente = sqlx::query(
                "SELECT id FROM finance.declaraciones
                 WHERE jurisdiccion = $1 AND tipo = $2 AND periodo_inicio = $3::date",
            )
            .bind(&jurisdiccion.codigo_pais)
            .bind(tipo)
            .bind(&mes.inicio)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
            if let Some(existente) = existente {
                let id: String = uuid_texto(&existente, "id").map_err(error_db)?;
                generadas.push(self.declaracion(&id).await?);
                continue;
            }

            let mut tx = self.db.transaccion().await.map_err(error_db)?;
            let fila = sqlx::query(
                "INSERT INTO finance.declaraciones
                   (jurisdiccion, tipo, periodo_inicio, periodo_fin,
                    base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                    iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                    ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en)
                 VALUES ($1, $2, $3::date, $4::date,
                         COALESCE($5, 0), COALESCE($6, 0), COALESCE($7, 0), COALESCE($8, 0),
                         COALESCE($9, 0), COALESCE($10, 0), COALESCE($11, 0), COALESCE($12, 0),
                         'GENERADA', $13, NOW())
                 RETURNING id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                           base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                           iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                           ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                           presentada_en, creado_en",
            )
            .bind(&jurisdiccion.codigo_pais)
            .bind(tipo)
            .bind(&mes.inicio)
            .bind(&mes.fin)
            .bind(campo(&campos, "base_gravada_cents"))
            .bind(campo(&campos, "iva_debitado_cents"))
            .bind(campo(&campos, "iva_credito_cents"))
            .bind(campo(&campos, "iva_a_pagar_cents"))
            .bind(campo(&campos, "renta_bruta_cents"))
            .bind(campo(&campos, "renta_gravable_cents"))
            .bind(campo(&campos, "ir_a_pagar_cents"))
            .bind(campo(&campos, "cuota_cents"))
            .bind(sqlx::types::Json(&detalle))
            .fetch_one(&mut *tx)
            .await
            .map_err(error_db)?;
            let creada = declaracion_de_fila(&fila);
            let a_pagar = match creada.tipo.as_str() {
                "IVA" => creada.iva_a_pagar_cents,
                "IR" => creada.ir_a_pagar_cents,
                _ => creada.cuota_cents,
            };
            self.outbox
                .insertar_en_transaccion(
                    &mut tx,
                    DECLARACION_GENERADA,
                    json!({
                        "declaracion_id": creada.id,
                        "jurisdiccion": creada.jurisdiccion,
                        "tipo": creada.tipo,
                        "periodo_inicio": mes.inicio,
                        "periodo_fin": mes.fin,
                        "a_pagar_cents": a_pagar,
                        "estado": creada.estado,
                    }),
                )
                .await?;
            tx.commit().await.map_err(error_db)?;
            generadas.push(creada);
        }
        Ok(generadas)
    }

    async fn declaracion(&self, id: &str) -> Result<Declaracion, ApiError> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "La declaracion debe ser un UUID."))?;
        let fila = sqlx::query(
            "SELECT id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                    base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                    iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                    ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                    presentada_en, creado_en
             FROM finance.declaraciones WHERE id = $1",
        )
        .bind(uuid)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?
        .ok_or_else(|| {
            ApiError::dominio(
                "DECLARACION_INEXISTENTE",
                format!("Declaracion {id} no existe."),
            )
        })?;
        Ok(declaracion_de_fila(&fila))
    }

    pub async fn presentar(&self, id: &str) -> Result<Option<Declaracion>, ApiError> {
        self.cambiar_estado(
            id,
            "UPDATE finance.declaraciones SET estado = 'PRESENTADA', presentada_en = NOW()
             WHERE id = $1 AND estado IN ('GENERADA', 'PENDIENTE')
             RETURNING id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                       base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                       iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                       ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                       presentada_en, creado_en",
        )
        .await
    }

    pub async fn marcar_pagada(&self, id: &str) -> Result<Option<Declaracion>, ApiError> {
        self.cambiar_estado(
            id,
            "UPDATE finance.declaraciones SET estado = 'PAGADA'
             WHERE id = $1 AND estado IN ('GENERADA', 'PRESENTADA')
             RETURNING id, jurisdiccion, tipo, periodo_inicio, periodo_fin,
                       base_gravada_cents, iva_debitado_cents, iva_credito_cents,
                       iva_a_pagar_cents, renta_bruta_cents, renta_gravable_cents,
                       ir_a_pagar_cents, cuota_cents, estado, detalle, generada_en,
                       presentada_en, creado_en",
        )
        .await
    }

    async fn cambiar_estado(&self, id: &str, sql: &str) -> Result<Option<Declaracion>, ApiError> {
        let uuid = uuid::Uuid::parse_str(id)
            .map_err(|_| ApiError::dominio("ID_INVALIDO", "La declaracion debe ser un UUID."))?;
        let fila = sqlx::query(sql)
            .bind(uuid)
            .fetch_optional(self.db.pool())
            .await
            .map_err(error_db)?;
        Ok(fila.as_ref().map(declaracion_de_fila))
    }
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
    Jurisdiccion {
        codigo_pais: fila.try_get("codigo_pais").unwrap_or_default(),
        nombre: fila.try_get("nombre").unwrap_or_default(),
        moneda: fila.try_get("moneda").unwrap_or_default(),
        simbolo_moneda: fila.try_get("simbolo_moneda").unwrap_or_default(),
        tasa_iva: fila.try_get::<f64, _>("tasa_iva").unwrap_or(0.0),
        tasa_ir: fila.try_get::<f64, _>("tasa_ir").unwrap_or(0.0),
        periodicidad_declaracion: fila.try_get("periodicidad_declaracion").unwrap_or_default(),
        leyes: jsonb(fila, "leyes").unwrap_or_else(|_| json!({})),
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
