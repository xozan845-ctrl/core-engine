//! Dominio de agenda: recursos, disponibilidad y bloqueos.
//!
//! Réplica de `scheduling.service.ts` (solo la parte de agenda) con el mismo
//! SQL, mensajes y códigos. Citas, recordatorios, Outbox y consumidores llegan
//! en el PR 4-5.

use crate::db::{
    es_conflicto_periodo, fecha, fecha_opcional, hora, jsonb, timestamp, uuid_texto, Db,
};
use crate::errors::ApiError;
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use serde_json::{json, Map, Value};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

const TIPOS_RECURSO: [&str; 5] = ["profesional", "sala", "sillon", "vehiculo", "otro"];

#[derive(Debug, Clone, PartialEq)]
pub struct CrearRecurso {
    pub tipo: String,
    pub nombre: String,
    pub atributos: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrearDisponibilidad {
    pub recurso_id: String,
    pub dia_semana: i64,
    pub inicia_hora: String,
    pub termina_hora: String,
    pub zona_horaria: Option<String>,
    pub vigente_desde: Option<String>,
    pub vigente_hasta: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrearBloqueo {
    pub recurso_id: String,
    pub inicia_en: String,
    pub termina_en: String,
    pub motivo: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerificarDisponibilidad {
    pub empresa_id: String,
    pub recurso_id: String,
    pub inicia_en: String,
    pub termina_en: String,
}

#[derive(Clone)]
pub struct Agenda {
    db: Arc<Db>,
}

impl Agenda {
    pub fn nueva(db: Arc<Db>) -> Self {
        Self { db }
    }

    // ── Recursos ────────────────────────────────────────────────────────────

    pub async fn crear_recurso(
        &self,
        empresa_id: &str,
        dto: &CrearRecurso,
    ) -> Result<Value, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "INSERT INTO scheduling.recursos (id, empresa_id, tipo, nombre, atributos)
             VALUES ($1, $2, $3, $4, $5::jsonb) RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(empresa)
        .bind(&dto.tipo)
        .bind(dto.nombre.trim())
        .bind(sqlx::types::Json(&dto.atributos))
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_recurso(&fila)
    }

    pub async fn listar_recursos(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT * FROM scheduling.recursos
             WHERE empresa_id = $1 ORDER BY estado ASC, nombre ASC",
        )
        .bind(empresa)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_recurso).collect()
    }

    // ── Disponibilidad ──────────────────────────────────────────────────────

    pub async fn crear_disponibilidad(
        &self,
        empresa_id: &str,
        dto: &CrearDisponibilidad,
    ) -> Result<Value, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let recurso = parsear_uuid(&dto.recurso_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        recurso_de_empresa(&mut tx, empresa, recurso).await?;
        let fila = sqlx::query(
            "INSERT INTO scheduling.disponibilidad
               (id, empresa_id, recurso_id, dia_semana, inicia_hora, termina_hora,
                zona_horaria, vigente_desde, vigente_hasta)
             VALUES ($1, $2, $3, $4, $5::time, $6::time, $7, COALESCE($8::date, CURRENT_DATE), $9::date)
             RETURNING *",
        )
        .bind(Uuid::new_v4())
        .bind(empresa)
        .bind(recurso)
        .bind(dto.dia_semana as i16)
        .bind(&dto.inicia_hora)
        .bind(&dto.termina_hora)
        .bind(dto.zona_horaria.as_deref().unwrap_or("America/Managua"))
        .bind(dto.vigente_desde.as_deref())
        .bind(dto.vigente_hasta.as_deref())
        .fetch_one(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        fila_disponibilidad(&fila)
    }

    pub async fn listar_disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let recurso = parsear_uuid(recurso_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT * FROM scheduling.disponibilidad
             WHERE empresa_id = $1 AND recurso_id = $2
             ORDER BY dia_semana, inicia_hora",
        )
        .bind(empresa)
        .bind(recurso)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_disponibilidad).collect()
    }

    // ── Bloqueos ────────────────────────────────────────────────────────────

    pub async fn crear_bloqueo(
        &self,
        empresa_id: &str,
        actor_id: Option<&str>,
        dto: &CrearBloqueo,
    ) -> Result<Value, ApiError> {
        validar_periodo(&dto.inicia_en, &dto.termina_en)?;
        let empresa = parsear_uuid(empresa_id)?;
        let recurso = parsear_uuid(&dto.recurso_id)?;
        let actor = match actor_id {
            Some(valor) => Some(parsear_uuid(valor)?),
            None => None,
        };

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        recurso_de_empresa_bloqueado(&mut tx, empresa, recurso).await?;
        let resultado = sqlx::query(
            "INSERT INTO scheduling.bloqueos
               (id, empresa_id, recurso_id, periodo, motivo, creado_por)
             VALUES ($1, $2, $3, tstzrange($4::timestamptz, $5::timestamptz, '[)'), $6, $7)
             RETURNING id, empresa_id, recurso_id, lower(periodo) AS inicia_en,
                       upper(periodo) AS termina_en, motivo, creado_en",
        )
        .bind(Uuid::new_v4())
        .bind(empresa)
        .bind(recurso)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .bind(dto.motivo.as_deref().unwrap_or(""))
        .bind(actor)
        .fetch_one(&mut *tx)
        .await;

        let fila = match resultado {
            Ok(fila) => fila,
            Err(error) if es_conflicto_periodo(&error) => {
                return Err(ApiError::recurso_no_disponible(
                    "El recurso ya tiene un bloqueo en ese horario.",
                ))
            }
            Err(error) => return Err(error_db(error)),
        };
        tx.commit().await.map_err(error_db)?;
        fila_bloqueo(&fila)
    }

    // ── Disponibilidad interna (orders) ─────────────────────────────────────

    pub async fn verificar_disponibilidad(
        &self,
        dto: &VerificarDisponibilidad,
    ) -> Result<(), ApiError> {
        validar_periodo(&dto.inicia_en, &dto.termina_en)?;
        let empresa = parsear_uuid(&dto.empresa_id)?;
        let recurso = parsear_uuid(&dto.recurso_id)?;

        let mut tx = self.db.transaccion().await.map_err(error_db)?;

        let existe = sqlx::query(
            "SELECT id FROM scheduling.recursos
              WHERE id = $1 AND empresa_id = $2 AND estado = 'activo'",
        )
        .bind(recurso)
        .bind(empresa)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if existe.is_none() {
            return Err(ApiError::no_encontrado("Recurso", &dto.recurso_id));
        }

        let ventana = sqlx::query(
            "SELECT 1 FROM scheduling.disponibilidad d
              WHERE d.recurso_id = $1
                AND d.dia_semana = EXTRACT(DOW FROM ($2::timestamptz AT TIME ZONE d.zona_horaria))::int
                AND d.inicia_hora <= (($2::timestamptz AT TIME ZONE d.zona_horaria)::time)
                AND d.termina_hora >= (($3::timestamptz AT TIME ZONE d.zona_horaria)::time)
                AND d.vigente_desde <= (($2::timestamptz AT TIME ZONE d.zona_horaria)::date)
                AND (d.vigente_hasta IS NULL OR d.vigente_hasta >= (($2::timestamptz AT TIME ZONE d.zona_horaria)::date))
              LIMIT 1",
        )
        .bind(recurso)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if ventana.is_none() {
            return Err(ApiError::recurso_no_disponible(
                "El recurso no tiene disponibilidad en ese horario.",
            ));
        }

        let bloqueo = sqlx::query(
            "SELECT 1 FROM scheduling.bloqueos
              WHERE recurso_id = $1
                AND periodo && tstzrange($2::timestamptz, $3::timestamptz, '[)') LIMIT 1",
        )
        .bind(recurso)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if bloqueo.is_some() {
            return Err(ApiError::recurso_no_disponible(
                "El recurso esta bloqueado en ese horario.",
            ));
        }

        let cita = sqlx::query(
            "SELECT 1 FROM scheduling.citas
              WHERE recurso_id = $1
                AND estado IN ('reservada', 'confirmada', 'en_atencion')
                AND periodo && tstzrange($2::timestamptz, $3::timestamptz, '[)') LIMIT 1",
        )
        .bind(recurso)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if cita.is_some() {
            return Err(ApiError::recurso_no_disponible(
                "El recurso ya esta reservado en ese horario.",
            ));
        }

        tx.commit().await.map_err(error_db)?;
        Ok(())
    }
}

/// Puerto de agenda para que los handlers no dependan de PostgreSQL en tests.
#[async_trait::async_trait]
pub trait AgendaPort: Send + Sync {
    async fn crear_recurso(&self, empresa_id: &str, dto: &CrearRecurso) -> Result<Value, ApiError>;
    async fn listar_recursos(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError>;
    async fn crear_disponibilidad(
        &self,
        empresa_id: &str,
        dto: &CrearDisponibilidad,
    ) -> Result<Value, ApiError>;
    async fn listar_disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
    ) -> Result<Vec<Value>, ApiError>;
    async fn crear_bloqueo(
        &self,
        empresa_id: &str,
        actor_id: Option<&str>,
        dto: &CrearBloqueo,
    ) -> Result<Value, ApiError>;
    async fn verificar_disponibilidad(&self, dto: &VerificarDisponibilidad)
        -> Result<(), ApiError>;
}

#[async_trait::async_trait]
impl AgendaPort for Agenda {
    async fn crear_recurso(&self, empresa_id: &str, dto: &CrearRecurso) -> Result<Value, ApiError> {
        Agenda::crear_recurso(self, empresa_id, dto).await
    }

    async fn listar_recursos(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        Agenda::listar_recursos(self, empresa_id).await
    }

    async fn crear_disponibilidad(
        &self,
        empresa_id: &str,
        dto: &CrearDisponibilidad,
    ) -> Result<Value, ApiError> {
        Agenda::crear_disponibilidad(self, empresa_id, dto).await
    }

    async fn listar_disponibilidad(
        &self,
        empresa_id: &str,
        recurso_id: &str,
    ) -> Result<Vec<Value>, ApiError> {
        Agenda::listar_disponibilidad(self, empresa_id, recurso_id).await
    }

    async fn crear_bloqueo(
        &self,
        empresa_id: &str,
        actor_id: Option<&str>,
        dto: &CrearBloqueo,
    ) -> Result<Value, ApiError> {
        Agenda::crear_bloqueo(self, empresa_id, actor_id, dto).await
    }

    async fn verificar_disponibilidad(
        &self,
        dto: &VerificarDisponibilidad,
    ) -> Result<(), ApiError> {
        Agenda::verificar_disponibilidad(self, dto).await
    }
}

/// Implementación para tests/arranque sin base de datos.
pub struct AgendaNoDisponible;

#[async_trait::async_trait]
impl AgendaPort for AgendaNoDisponible {
    async fn crear_recurso(&self, _: &str, _: &CrearRecurso) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn listar_recursos(&self, _: &str) -> Result<Vec<Value>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn crear_disponibilidad(
        &self,
        _: &str,
        _: &CrearDisponibilidad,
    ) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn listar_disponibilidad(&self, _: &str, _: &str) -> Result<Vec<Value>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn crear_bloqueo(
        &self,
        _: &str,
        _: Option<&str>,
        _: &CrearBloqueo,
    ) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn verificar_disponibilidad(&self, _: &VerificarDisponibilidad) -> Result<(), ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
}

async fn recurso_de_empresa(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa: Uuid,
    recurso: Uuid,
) -> Result<(), ApiError> {
    let fila = sqlx::query("SELECT id FROM scheduling.recursos WHERE id = $1 AND empresa_id = $2")
        .bind(recurso)
        .bind(empresa)
        .fetch_optional(&mut **tx)
        .await
        .map_err(error_db)?;
    if fila.is_none() {
        return Err(ApiError::no_encontrado("Recurso", &recurso.to_string()));
    }
    Ok(())
}

async fn recurso_de_empresa_bloqueado(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa: Uuid,
    recurso: Uuid,
) -> Result<(), ApiError> {
    let fila = sqlx::query(
        "SELECT id FROM scheduling.recursos WHERE id = $1 AND empresa_id = $2 FOR UPDATE",
    )
    .bind(recurso)
    .bind(empresa)
    .fetch_optional(&mut **tx)
    .await
    .map_err(error_db)?;
    if fila.is_none() {
        return Err(ApiError::no_encontrado("Recurso", &recurso.to_string()));
    }
    Ok(())
}

fn fila_recurso(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
        "tipo": fila.try_get::<String, _>("tipo").map_err(error_db)?,
        "nombre": fila.try_get::<String, _>("nombre").map_err(error_db)?,
        "estado": fila.try_get::<String, _>("estado").map_err(error_db)?,
        "atributos": jsonb(fila, "atributos").map_err(error_db)?,
        "creado_en": timestamp(fila, "creado_en").map_err(error_db)?,
        "actualizado_en": timestamp(fila, "actualizado_en").map_err(error_db)?,
    }))
}

fn fila_disponibilidad(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
        "recurso_id": uuid_texto(fila, "recurso_id").map_err(error_db)?,
        "dia_semana": fila.try_get::<i16, _>("dia_semana").map_err(error_db)?,
        "inicia_hora": hora(fila, "inicia_hora").map_err(error_db)?,
        "termina_hora": hora(fila, "termina_hora").map_err(error_db)?,
        "zona_horaria": fila.try_get::<String, _>("zona_horaria").map_err(error_db)?,
        "vigente_desde": fecha(fila, "vigente_desde").map_err(error_db)?,
        "vigente_hasta": fecha_opcional(fila, "vigente_hasta").map_err(error_db)?,
        "creado_en": timestamp(fila, "creado_en").map_err(error_db)?,
    }))
}

fn fila_bloqueo(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
        "recurso_id": uuid_texto(fila, "recurso_id").map_err(error_db)?,
        "inicia_en": timestamp(fila, "inicia_en").map_err(error_db)?,
        "termina_en": timestamp(fila, "termina_en").map_err(error_db)?,
        "motivo": fila.try_get::<String, _>("motivo").map_err(error_db)?,
        "creado_en": timestamp(fila, "creado_en").map_err(error_db)?,
    }))
}

// ── Errores y conversiones ─────────────────────────────────────────────────

pub fn error_db(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "scheduling",
        crate::campos! { "msg": "Error de base de datos", "err": error.to_string() },
    );
    ApiError::interno()
}

pub fn parsear_uuid(valor: &str) -> Result<Uuid, ApiError> {
    Uuid::parse_str(valor).map_err(|_| ApiError::interno())
}

// ── Validación (equivalente a DtoValidationPipe + class-validator) ─────────

fn error_campo(campo: &str, restricciones: &[&str]) -> Value {
    json!({ "campo": campo, "restricciones": restricciones })
}

fn error_validacion(errores: Vec<Value>) -> ApiError {
    ApiError::validacion(Value::Array(errores))
}

pub fn es_uuid(valor: &str) -> bool {
    let bytes = valor.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (indice, byte) in bytes.iter().enumerate() {
        match indice {
            8 | 13 | 18 | 23 => {
                if *byte != b'-' {
                    return false;
                }
            }
            14 => {
                if !(b'1'..=b'5').contains(byte) {
                    return false;
                }
            }
            19 => {
                if !matches!(byte, b'8' | b'9' | b'a' | b'b' | b'A' | b'B') {
                    return false;
                }
            }
            _ => {
                if !byte.is_ascii_hexdigit() {
                    return false;
                }
            }
        }
    }
    true
}

/// `IsDateString` (validator.js isISO8601): fecha, fecha-hora o RFC 3339.
pub fn es_iso8601(valor: &str) -> bool {
    DateTime::parse_from_rfc3339(valor).is_ok()
        || NaiveDate::parse_from_str(valor, "%Y-%m-%d").is_ok()
        || NaiveDateTime::parse_from_str(valor, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
        || NaiveDateTime::parse_from_str(valor, "%Y-%m-%dT%H:%M:%S").is_ok()
}

fn instante_ms(valor: &str) -> Option<i64> {
    if let Ok(fecha) = DateTime::parse_from_rfc3339(valor) {
        return Some(fecha.timestamp_millis());
    }
    if let Ok(fecha) = NaiveDateTime::parse_from_str(valor, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(Utc.from_utc_datetime(&fecha).timestamp_millis());
    }
    if let Ok(fecha) = NaiveDate::parse_from_str(valor, "%Y-%m-%d") {
        return fecha
            .and_hms_opt(0, 0, 0)
            .map(|fecha| Utc.from_utc_datetime(&fecha).timestamp_millis());
    }
    None
}

pub fn validar_periodo(inicia: &str, termina: &str) -> Result<(), ApiError> {
    let Some(inicio_ms) = instante_ms(inicia) else {
        return Err(periodo_invalido());
    };
    let Some(fin_ms) = instante_ms(termina) else {
        return Err(periodo_invalido());
    };
    if inicio_ms >= fin_ms {
        return Err(periodo_invalido());
    }
    Ok(())
}

fn periodo_invalido() -> ApiError {
    ApiError::dominio(
        "PERIODO_INVALIDO",
        "La hora de inicio debe ser anterior a la hora de finalizacion.",
    )
}

fn objeto(valor: &Value) -> Map<String, Value> {
    valor.as_object().cloned().unwrap_or_default()
}

fn opcional_iso(
    objeto: &Map<String, Value>,
    campo: &str,
    errores: &mut Vec<Value>,
) -> Option<String> {
    match objeto.get(campo) {
        None | Some(Value::Null) => None,
        Some(Value::String(valor)) if es_iso8601(valor) => Some(valor.clone()),
        Some(_) => {
            errores.push(error_campo(campo, &["isDateString"]));
            None
        }
    }
}

fn opcional_string(
    objeto: &Map<String, Value>,
    campo: &str,
    errores: &mut Vec<Value>,
) -> Option<String> {
    match objeto.get(campo) {
        None | Some(Value::Null) => None,
        Some(Value::String(valor)) => Some(valor.clone()),
        Some(_) => {
            errores.push(error_campo(campo, &["isString"]));
            None
        }
    }
}

pub fn validar_crear_recurso(valor: &Value) -> Result<CrearRecurso, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let tipo = match objeto.get("tipo").and_then(Value::as_str) {
        Some(valor) if TIPOS_RECURSO.contains(&valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("tipo", &["isIn"]));
            String::new()
        }
    };

    let nombre = match objeto.get("nombre").and_then(Value::as_str) {
        Some(valor) => valor.to_string(),
        None => {
            errores.push(error_campo("nombre", &["isString"]));
            String::new()
        }
    };

    let atributos = match objeto.get("atributos") {
        None | Some(Value::Null) => json!({}),
        Some(valor @ Value::Object(_)) => valor.clone(),
        Some(_) => {
            errores.push(error_campo("atributos", &["isObject"]));
            json!({})
        }
    };

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(CrearRecurso {
        tipo,
        nombre,
        atributos,
    })
}

pub fn validar_crear_disponibilidad(valor: &Value) -> Result<CrearDisponibilidad, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let recurso_id = match objeto.get("recurso_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("recurso_id", &["isUuid"]));
            String::new()
        }
    };

    let dia_semana = match objeto.get("dia_semana").and_then(Value::as_i64) {
        Some(valor) => {
            if valor < 0 {
                errores.push(error_campo("dia_semana", &["min"]));
            } else if valor > 6 {
                errores.push(error_campo("dia_semana", &["max"]));
            }
            valor
        }
        None => {
            errores.push(error_campo("dia_semana", &["isInt"]));
            0
        }
    };

    let inicia_hora = match objeto.get("inicia_hora").and_then(Value::as_str) {
        Some(valor) => valor.to_string(),
        None => {
            errores.push(error_campo("inicia_hora", &["isString"]));
            String::new()
        }
    };
    let termina_hora = match objeto.get("termina_hora").and_then(Value::as_str) {
        Some(valor) => valor.to_string(),
        None => {
            errores.push(error_campo("termina_hora", &["isString"]));
            String::new()
        }
    };

    let zona_horaria = opcional_string(&objeto, "zona_horaria", &mut errores);
    let vigente_desde = opcional_iso(&objeto, "vigente_desde", &mut errores);
    let vigente_hasta = opcional_iso(&objeto, "vigente_hasta", &mut errores);

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(CrearDisponibilidad {
        recurso_id,
        dia_semana,
        inicia_hora,
        termina_hora,
        zona_horaria,
        vigente_desde,
        vigente_hasta,
    })
}

pub fn validar_crear_bloqueo(valor: &Value) -> Result<CrearBloqueo, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let recurso_id = match objeto.get("recurso_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("recurso_id", &["isUuid"]));
            String::new()
        }
    };
    let inicia_en = match objeto.get("inicia_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("inicia_en", &["isDateString"]));
            String::new()
        }
    };
    let termina_en = match objeto.get("termina_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("termina_en", &["isDateString"]));
            String::new()
        }
    };
    let motivo = opcional_string(&objeto, "motivo", &mut errores);

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(CrearBloqueo {
        recurso_id,
        inicia_en,
        termina_en,
        motivo,
    })
}

pub fn validar_verificar_disponibilidad(
    valor: &Value,
) -> Result<VerificarDisponibilidad, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let empresa_id = match objeto.get("empresa_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("empresa_id", &["isUuid"]));
            String::new()
        }
    };
    let recurso_id = match objeto.get("recurso_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("recurso_id", &["isUuid"]));
            String::new()
        }
    };
    let inicia_en = match objeto.get("inicia_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("inicia_en", &["isDateString"]));
            String::new()
        }
    };
    let termina_en = match objeto.get("termina_en").and_then(Value::as_str) {
        Some(valor) if es_iso8601(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("termina_en", &["isDateString"]));
            String::new()
        }
    };

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(VerificarDisponibilidad {
        empresa_id,
        recurso_id,
        inicia_en,
        termina_en,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "123e4567-e89b-42d3-a456-426614174000";

    #[test]
    fn debe_aceptar_el_recurso_valido_y_normalizar_atributos() {
        let dto = validar_crear_recurso(&json!({
            "tipo": "sala", "nombre": " Sala 1 ", "atributos": {"piso": 2}
        }))
        .expect("valido");
        assert_eq!(dto.tipo, "sala");
        assert_eq!(dto.nombre, " Sala 1 ");
        assert_eq!(dto.atributos["piso"], 2);
    }

    #[test]
    fn debe_reportar_las_restricciones_en_orden_de_declaracion() {
        let error = validar_crear_recurso(&json!({ "tipo": "nave", "atributos": [] }))
            .expect_err("invalido");
        assert_eq!(
            error.cuerpo()["detalles"],
            json!([
                {"campo": "tipo", "restricciones": ["isIn"]},
                {"campo": "nombre", "restricciones": ["isString"]},
                {"campo": "atributos", "restricciones": ["isObject"]},
            ])
        );
    }

    #[test]
    fn debe_validar_uuid_con_version_y_variante() {
        assert!(es_uuid(UUID));
        assert!(!es_uuid("123e4567-e89b-02d3-a456-426614174000"));
        assert!(!es_uuid("123e4567-e89b-42d3-c456-426614174000"));
        assert!(!es_uuid("no-es-uuid"));
    }

    #[test]
    fn debe_rechazar_dia_semana_fuera_de_rango_con_min_o_max() {
        let fuera = validar_crear_disponibilidad(&json!({
            "recurso_id": UUID, "dia_semana": 7, "inicia_hora": "08:00", "termina_hora": "17:00"
        }))
        .expect_err("invalido");
        assert_eq!(
            fuera.cuerpo()["detalles"],
            json!([{"campo": "dia_semana", "restricciones": ["max"]}])
        );
    }

    #[test]
    fn debe_validar_las_fechas_opcionales_como_iso8601() {
        let error = validar_crear_disponibilidad(&json!({
            "recurso_id": UUID, "dia_semana": 1, "inicia_hora": "08:00", "termina_hora": "17:00",
            "vigente_desde": "2026-13-01"
        }))
        .expect_err("invalido");
        assert_eq!(
            error.cuerpo()["detalles"],
            json!([{"campo": "vigente_desde", "restricciones": ["isDateString"]}])
        );
    }

    #[test]
    fn debe_validar_el_periodo_igual_que_date_parse() {
        assert!(validar_periodo("2030-04-15T14:00:00.000Z", "2030-04-15T15:00:00.000Z").is_ok());
        assert_eq!(
            validar_periodo("2030-04-15T15:00:00Z", "2030-04-15T14:00:00Z")
                .expect_err("invalido")
                .codigo,
            "PERIODO_INVALIDO"
        );
        assert_eq!(
            validar_periodo("no-fecha", "2030-04-15T14:00:00Z")
                .expect_err("invalido")
                .codigo,
            "PERIODO_INVALIDO"
        );
    }

    #[test]
    fn debe_aceptar_un_cuerpo_vacio_como_objeto_sin_campos() {
        let error = validar_crear_bloqueo(&Value::Null).expect_err("invalido");
        assert_eq!(
            error.cuerpo()["detalles"],
            json!([
                {"campo": "recurso_id", "restricciones": ["isUuid"]},
                {"campo": "inicia_en", "restricciones": ["isDateString"]},
                {"campo": "termina_en", "restricciones": ["isDateString"]},
            ])
        );
    }
}
