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
    pub capacidad: i32,
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
}

mod citas;
mod helpers;
mod puerto;
mod recordatorios;
mod recursos;

pub(crate) use helpers::*;
pub use puerto::{AgendaNoDisponible, AgendaPort};

#[cfg(test)]
mod tests;
