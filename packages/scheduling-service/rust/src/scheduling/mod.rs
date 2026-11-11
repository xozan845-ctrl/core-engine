//! Dominio de agenda: recursos, disponibilidad y bloqueos.
//!
//! Réplica de `scheduling.service.ts` (solo la parte de agenda) con el mismo
//! SQL, mensajes y códigos. Citas, recordatorios, Outbox y consumidores llegan
//! en el PR 4-5.

use crate::clients::Partes;
use crate::db::{
    es_conflicto_periodo, fecha, fecha_opcional, hora, jsonb, timestamp, timestamp_opcional,
    uuid_opcional, uuid_texto, Db,
};
use crate::errors::ApiError;
use crate::outbox::{Outbox, APPOINTMENT_CANCELLED, APPOINTMENT_CREATED, APPOINTMENT_REMINDER_DUE};
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
pub struct CrearCita {
    pub recurso_id: String,
    pub parte_id: String,
    pub order_id: Option<String>,
    pub item_id: Option<String>,
    pub inicia_en: String,
    pub termina_en: String,
    pub notas: Option<String>,
    pub metadata: Value,
}

/// Línea de una orden que puede materializarse como cita (evento `order.created`).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct OrdenItem {
    pub empresa_id: String,
    pub item_type: String,
    pub item_id: Option<String>,
    pub fulfillment: Option<String>,
    #[serde(default)]
    pub fulfillment_config: Option<Value>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OrdenReserva {
    pub order_id: String,
    pub cliente_parte_id: String,
    #[serde(default)]
    pub items: Vec<OrdenItem>,
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
    partes: Arc<dyn Partes>,
    outbox: Arc<Outbox>,
}

impl Agenda {
    pub fn nueva(db: Arc<Db>, partes: Arc<dyn Partes>, outbox_tabla: &str) -> Self {
        Self {
            db,
            partes,
            outbox: Arc::new(Outbox::nueva(outbox_tabla)),
        }
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

    // ── Citas ───────────────────────────────────────────────────────────────

    pub async fn crear_cita(&self, empresa_id: &str, dto: &CrearCita) -> Result<Value, ApiError> {
        validar_periodo(&dto.inicia_en, &dto.termina_en)?;
        let empresa = parsear_uuid(empresa_id)?;
        let recurso = parsear_uuid(&dto.recurso_id)?;
        match self.partes.pertenece(empresa_id, &dto.parte_id).await {
            Err(()) => return Err(ApiError::servicio_no_disponible("identity-service")),
            Ok(false) => {
                return Err(ApiError::dominio(
                    "PARTE_INVALIDA",
                    "La parte no pertenece al tenant activo.",
                ))
            }
            Ok(true) => {}
        }
        let order_id = dto.order_id.as_deref().map(parsear_uuid).transpose()?;
        let item_id = dto.item_id.as_deref().map(parsear_uuid).transpose()?;

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let recurso_activo = sqlx::query(
            "SELECT id FROM scheduling.recursos
             WHERE id = $1 AND empresa_id = $2 AND estado = 'activo' FOR UPDATE",
        )
        .bind(recurso)
        .bind(empresa)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?;
        if recurso_activo.is_none() {
            return Err(ApiError::no_encontrado("Recurso", &dto.recurso_id));
        }
        validar_disponibilidad_tx(&mut tx, recurso, &dto.inicia_en, &dto.termina_en).await?;
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
                "El recurso está bloqueado en ese horario.",
            ));
        }

        let id = Uuid::new_v4();
        let cita = sqlx::query(
            "INSERT INTO scheduling.citas
               (id, empresa_id, recurso_id, parte_id, order_id, item_id,
                inicia_en, termina_en, notas, metadata)
             VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz, $8::timestamptz, $9, $10::jsonb)
             RETURNING id, empresa_id, recurso_id, parte_id, order_id, item_id,
                       estado, inicia_en, termina_en, notas, metadata, creado_en, actualizado_en",
        )
        .bind(id)
        .bind(empresa)
        .bind(recurso)
        .bind(parsear_uuid(&dto.parte_id)?)
        .bind(order_id)
        .bind(item_id)
        .bind(&dto.inicia_en)
        .bind(&dto.termina_en)
        .bind(dto.notas.as_deref().unwrap_or(""))
        .bind(sqlx::types::Json(&dto.metadata))
        .fetch_one(&mut *tx)
        .await
        .map_err(|error| {
            if es_conflicto_periodo(&error) {
                ApiError::recurso_no_disponible("El recurso ya está reservado en ese horario.")
            } else {
                error_db(error)
            }
        })?;

        crear_recordatorios(&mut tx, empresa, id, &dto.inicia_en).await?;

        let mut data = json!({
            "cita_id": uuid_texto(&cita, "id").map_err(error_db)?,
            "empresa_id": uuid_texto(&cita, "empresa_id").map_err(error_db)?,
            "recurso_id": uuid_texto(&cita, "recurso_id").map_err(error_db)?,
        });
        if let Some(parte) = uuid_opcional(&cita, "parte_id").map_err(error_db)? {
            data["cliente_parte_id"] = json!(parte);
        }
        if let Some(order) = uuid_opcional(&cita, "order_id").map_err(error_db)? {
            data["order_id"] = json!(order);
        }
        if let Some(item) = uuid_opcional(&cita, "item_id").map_err(error_db)? {
            data["item_id"] = json!(item);
        }
        data["inicia_en"] = json!(timestamp(&cita, "inicia_en").map_err(error_db)?);
        data["termina_en"] = json!(timestamp(&cita, "termina_en").map_err(error_db)?);
        data["estado"] = json!(cita.try_get::<String, _>("estado").map_err(error_db)?);
        self.outbox
            .insertar_en_transaccion(&mut tx, APPOINTMENT_CREATED, data)
            .await?;

        tx.commit().await.map_err(error_db)?;
        fila_cita(&cita)
    }

    pub async fn listar_citas(
        &self,
        empresa_id: &str,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT id, empresa_id, recurso_id, parte_id, order_id, item_id, estado,
                    inicia_en, termina_en, notas, metadata, creado_en, actualizado_en
             FROM scheduling.citas
             WHERE empresa_id = $1
               AND ($2::timestamptz IS NULL OR termina_en > $2::timestamptz)
               AND ($3::timestamptz IS NULL OR inicia_en < $3::timestamptz)
             ORDER BY inicia_en ASC LIMIT 500",
        )
        .bind(empresa)
        .bind(desde)
        .bind(hasta)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_cita).collect()
    }

    pub async fn cancelar_cita(
        &self,
        empresa_id: &str,
        cita_id: &str,
        motivo: Option<&str>,
    ) -> Result<Value, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let cita = parsear_uuid(cita_id)?;
        let estados: Vec<String> = ESTADOS_ACTIVOS.iter().map(|e| e.to_string()).collect();

        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let fila = sqlx::query(
            "UPDATE scheduling.citas
                SET estado = 'cancelada', actualizado_en = NOW()
              WHERE id = $1 AND empresa_id = $2
                AND estado = ANY($3::text[])
             RETURNING id, empresa_id, recurso_id, parte_id, order_id, item_id, estado,
                       inicia_en, termina_en, notas, metadata, creado_en, actualizado_en",
        )
        .bind(cita)
        .bind(empresa)
        .bind(&estados)
        .fetch_optional(&mut *tx)
        .await
        .map_err(error_db)?
        .ok_or_else(|| ApiError::no_encontrado("Cita activa", cita_id))?;

        sqlx::query(
            "UPDATE scheduling.recordatorios SET estado = 'cancelado'
             WHERE cita_id = $1 AND estado = 'pendiente'",
        )
        .bind(cita)
        .execute(&mut *tx)
        .await
        .map_err(error_db)?;

        let mut data = json!({
            "cita_id": uuid_texto(&fila, "id").map_err(error_db)?,
            "empresa_id": uuid_texto(&fila, "empresa_id").map_err(error_db)?,
        });
        if let Some(order) = uuid_opcional(&fila, "order_id").map_err(error_db)? {
            data["order_id"] = json!(order);
        }
        if let Some(motivo) = motivo.filter(|valor| !valor.is_empty()) {
            data["motivo"] = json!(motivo);
        }
        self.outbox
            .insertar_en_transaccion(&mut tx, APPOINTMENT_CANCELLED, data)
            .await?;

        tx.commit().await.map_err(error_db)?;
        fila_cita(&fila)
    }

    pub async fn listar_recordatorios(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        let empresa = parsear_uuid(empresa_id)?;
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT id, cita_id, tipo, programado_en, estado, enviado_en
             FROM scheduling.recordatorios
             WHERE empresa_id = $1 ORDER BY programado_en ASC LIMIT 500",
        )
        .bind(empresa)
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;
        tx.commit().await.map_err(error_db)?;
        filas.iter().map(fila_recordatorio).collect()
    }

    /// Materializa una reserva de `order.created` (idempotente por `order_id`).
    pub async fn crear_desde_reserva(&self, orden: &OrdenReserva) -> Result<Value, ApiError> {
        let order_id = parsear_uuid(&orden.order_id)?;
        let existente = sqlx::query(
            "SELECT id, empresa_id, recurso_id, parte_id, order_id, item_id, estado,
                    inicia_en, termina_en, notas, metadata, creado_en, actualizado_en
             FROM scheduling.citas WHERE order_id = $1",
        )
        .bind(order_id)
        .fetch_optional(self.db.pool())
        .await
        .map_err(error_db)?;
        if let Some(fila) = existente {
            return fila_cita(&fila);
        }

        let item = orden.items.iter().find(|item| {
            matches!(item.item_type.as_str(), "servicio" | "alquiler")
                && item.fulfillment.as_deref() == Some("onsite")
        });
        let Some(item) = item else {
            return Err(configuracion_requerida());
        };
        let config = item
            .fulfillment_config
            .clone()
            .filter(Value::is_object)
            .unwrap_or_else(|| json!({}));
        let (Some(recurso_id), Some(inicia_en), Some(termina_en)) = (
            texto_config(&config, "recurso_id"),
            texto_config(&config, "inicia_en"),
            texto_config(&config, "termina_en"),
        ) else {
            return Err(configuracion_requerida());
        };

        let mut metadata = json!({ "origen": "order.created" });
        if let Some(reserva) = config.get("metadata").filter(|valor| valor.is_object()) {
            metadata["reserva"] = reserva.clone();
        }

        self.crear_cita(
            &item.empresa_id,
            &CrearCita {
                recurso_id,
                parte_id: orden.cliente_parte_id.clone(),
                order_id: Some(orden.order_id.clone()),
                item_id: item.item_id.clone(),
                inicia_en,
                termina_en,
                notas: texto_config(&config, "notas"),
                metadata,
            },
        )
        .await
    }

    /// Recordatorios vencidos: los marca enviados y publica el evento (PR 5).
    pub async fn procesar_recordatorios(&self) -> Result<usize, ApiError> {
        let mut tx = self.db.transaccion().await.map_err(error_db)?;
        let filas = sqlx::query(
            "SELECT r.id, r.cita_id, r.empresa_id, r.tipo, r.programado_en
             FROM scheduling.recordatorios r
             WHERE r.estado = 'pendiente' AND r.programado_en <= NOW()
             ORDER BY r.programado_en ASC LIMIT 100 FOR UPDATE SKIP LOCKED",
        )
        .fetch_all(&mut *tx)
        .await
        .map_err(error_db)?;

        for fila in &filas {
            let recordatorio_id = uuid_texto(fila, "id").map_err(error_db)?;
            sqlx::query(
                "UPDATE scheduling.recordatorios
                    SET estado = 'enviado', enviado_en = NOW()
                  WHERE id = $1 AND estado = 'pendiente'",
            )
            .bind(parsear_uuid(&recordatorio_id)?)
            .execute(&mut *tx)
            .await
            .map_err(error_db)?;

            let data = json!({
                "recordatorio_id": recordatorio_id,
                "cita_id": uuid_texto(fila, "cita_id").map_err(error_db)?,
                "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
                "tipo": fila.try_get::<String, _>("tipo").map_err(error_db)?,
                "programado_en": timestamp(fila, "programado_en").map_err(error_db)?,
            });
            self.outbox
                .insertar_en_transaccion(&mut tx, APPOINTMENT_REMINDER_DUE, data)
                .await?;
        }

        let total = filas.len();
        tx.commit().await.map_err(error_db)?;
        Ok(total)
    }
}

const ESTADOS_ACTIVOS: [&str; 3] = ["reservada", "confirmada", "en_atencion"];

async fn validar_disponibilidad_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    recurso: Uuid,
    inicia: &str,
    termina: &str,
) -> Result<(), ApiError> {
    let disponible = sqlx::query(
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
    .bind(inicia)
    .bind(termina)
    .fetch_optional(&mut **tx)
    .await
    .map_err(error_db)?;
    if disponible.is_none() {
        return Err(ApiError::recurso_no_disponible(
            "El recurso no tiene disponibilidad en ese horario.",
        ));
    }
    Ok(())
}

async fn crear_recordatorios(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    empresa: Uuid,
    cita: Uuid,
    inicia_en: &str,
) -> Result<(), ApiError> {
    sqlx::query(
        "INSERT INTO scheduling.recordatorios (id, empresa_id, cita_id, tipo, programado_en)
         SELECT gen_random_uuid(), $1, $2, tipo, ($3::timestamptz - intervalo)
         FROM (VALUES ('24h'::text, INTERVAL '24 hours'), ('2h'::text, INTERVAL '2 hours')) AS x(tipo, intervalo)
         WHERE ($3::timestamptz - intervalo) > NOW()
         ON CONFLICT (cita_id, tipo) DO NOTHING",
    )
    .bind(empresa)
    .bind(cita)
    .bind(inicia_en)
    .execute(&mut **tx)
    .await
    .map_err(error_db)?;
    Ok(())
}

fn fila_cita(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "empresa_id": uuid_texto(fila, "empresa_id").map_err(error_db)?,
        "recurso_id": uuid_texto(fila, "recurso_id").map_err(error_db)?,
        "parte_id": uuid_opcional(fila, "parte_id").map_err(error_db)?,
        "order_id": uuid_opcional(fila, "order_id").map_err(error_db)?,
        "item_id": uuid_opcional(fila, "item_id").map_err(error_db)?,
        "estado": fila.try_get::<String, _>("estado").map_err(error_db)?,
        "inicia_en": timestamp(fila, "inicia_en").map_err(error_db)?,
        "termina_en": timestamp(fila, "termina_en").map_err(error_db)?,
        "notas": fila.try_get::<String, _>("notas").map_err(error_db)?,
        "metadata": jsonb(fila, "metadata").map_err(error_db)?,
        "creado_en": timestamp(fila, "creado_en").map_err(error_db)?,
        "actualizado_en": timestamp(fila, "actualizado_en").map_err(error_db)?,
    }))
}

fn fila_recordatorio(fila: &sqlx::postgres::PgRow) -> Result<Value, ApiError> {
    Ok(json!({
        "id": uuid_texto(fila, "id").map_err(error_db)?,
        "cita_id": uuid_texto(fila, "cita_id").map_err(error_db)?,
        "tipo": fila.try_get::<String, _>("tipo").map_err(error_db)?,
        "programado_en": timestamp(fila, "programado_en").map_err(error_db)?,
        "estado": fila.try_get::<String, _>("estado").map_err(error_db)?,
        "enviado_en": timestamp_opcional(fila, "enviado_en").map_err(error_db)?,
    }))
}

fn configuracion_requerida() -> ApiError {
    ApiError::dominio(
        "CITA_CONFIGURACION_REQUERIDA",
        "Una reserva requiere un servicio onsite con recurso_id, inicia_en y termina_en.",
    )
}

fn texto_config(config: &Value, clave: &str) -> Option<String> {
    config
        .get(clave)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
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
    async fn crear_cita(&self, empresa_id: &str, dto: &CrearCita) -> Result<Value, ApiError>;
    async fn listar_citas(
        &self,
        empresa_id: &str,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Result<Vec<Value>, ApiError>;
    async fn cancelar_cita(
        &self,
        empresa_id: &str,
        cita_id: &str,
        motivo: Option<&str>,
    ) -> Result<Value, ApiError>;
    async fn listar_recordatorios(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError>;
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

    async fn crear_cita(&self, empresa_id: &str, dto: &CrearCita) -> Result<Value, ApiError> {
        Agenda::crear_cita(self, empresa_id, dto).await
    }

    async fn listar_citas(
        &self,
        empresa_id: &str,
        desde: Option<&str>,
        hasta: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        Agenda::listar_citas(self, empresa_id, desde, hasta).await
    }

    async fn cancelar_cita(
        &self,
        empresa_id: &str,
        cita_id: &str,
        motivo: Option<&str>,
    ) -> Result<Value, ApiError> {
        Agenda::cancelar_cita(self, empresa_id, cita_id, motivo).await
    }

    async fn listar_recordatorios(&self, empresa_id: &str) -> Result<Vec<Value>, ApiError> {
        Agenda::listar_recordatorios(self, empresa_id).await
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
    async fn crear_cita(&self, _: &str, _: &CrearCita) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn listar_citas(
        &self,
        _: &str,
        _: Option<&str>,
        _: Option<&str>,
    ) -> Result<Vec<Value>, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn cancelar_cita(&self, _: &str, _: &str, _: Option<&str>) -> Result<Value, ApiError> {
        Err(ApiError::servicio_no_disponible("postgres"))
    }
    async fn listar_recordatorios(&self, _: &str) -> Result<Vec<Value>, ApiError> {
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

fn opcional_uuid(
    objeto: &Map<String, Value>,
    campo: &str,
    errores: &mut Vec<Value>,
) -> Option<String> {
    match objeto.get(campo) {
        None | Some(Value::Null) => None,
        Some(Value::String(valor)) if es_uuid(valor) => Some(valor.clone()),
        Some(_) => {
            errores.push(error_campo(campo, &["isUuid"]));
            None
        }
    }
}

pub fn validar_crear_cita(valor: &Value) -> Result<CrearCita, ApiError> {
    let objeto = objeto(valor);
    let mut errores = Vec::new();

    let recurso_id = match objeto.get("recurso_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("recurso_id", &["isUuid"]));
            String::new()
        }
    };
    let parte_id = match objeto.get("parte_id").and_then(Value::as_str) {
        Some(valor) if es_uuid(valor) => valor.to_string(),
        _ => {
            errores.push(error_campo("parte_id", &["isUuid"]));
            String::new()
        }
    };
    let order_id = opcional_uuid(&objeto, "order_id", &mut errores);
    let item_id = opcional_uuid(&objeto, "item_id", &mut errores);

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
    let notas = opcional_string(&objeto, "notas", &mut errores);
    let metadata = match objeto.get("metadata") {
        None | Some(Value::Null) => json!({}),
        Some(valor @ Value::Object(_)) => valor.clone(),
        Some(_) => {
            errores.push(error_campo("metadata", &["isObject"]));
            json!({})
        }
    };

    if !errores.is_empty() {
        return Err(error_validacion(errores));
    }
    Ok(CrearCita {
        recurso_id,
        parte_id,
        order_id,
        item_id,
        inicia_en,
        termina_en,
        notas,
        metadata,
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

    #[test]
    fn debe_validar_la_cita_como_el_dto_original() {
        let valida = validar_crear_cita(&json!({
            "recurso_id": UUID,
            "parte_id": "22222222-2222-4222-8222-222222222222",
            "order_id": "33333333-3333-4333-8333-333333333333",
            "inicia_en": "2030-04-15T14:00:00.000Z",
            "termina_en": "2030-04-15T15:00:00.000Z",
            "metadata": {"canal": "web"}
        }))
        .expect("valida");
        assert_eq!(valida.metadata["canal"], "web");
        assert_eq!(valida.notas, None);

        let invalida = validar_crear_cita(&json!({
            "recurso_id": "no-uuid",
            "order_id": "tampoco",
            "inicia_en": "2030-04-15",
            "metadata": []
        }))
        .expect_err("invalida");
        assert_eq!(
            invalida.cuerpo()["detalles"],
            json!([
                {"campo": "recurso_id", "restricciones": ["isUuid"]},
                {"campo": "parte_id", "restricciones": ["isUuid"]},
                {"campo": "order_id", "restricciones": ["isUuid"]},
                {"campo": "termina_en", "restricciones": ["isDateString"]},
                {"campo": "metadata", "restricciones": ["isObject"]},
            ])
        );
    }
}
