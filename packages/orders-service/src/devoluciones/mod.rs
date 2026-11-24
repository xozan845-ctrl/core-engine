//! RMA: devoluciones parciales por línea.
//!
//! La devolución total sigue siendo una transición de la orden. Esta entidad
//! permite devolver cantidades concretas, restockear en inventory y emitir el
//! evento que compensa la comisión proporcional.

mod estados;
mod reembolsos;
mod solicitudes;

mod prelude {
    pub(crate) use crate::auth::UsuarioContexto;
    pub(crate) use crate::clients::ClientesPort;
    pub(crate) use crate::db::{iso_ms, uuid_opcional, uuid_texto, Db};
    pub(crate) use crate::errors::ApiError;
    pub(crate) use crate::outbox::Outbox;
    pub(crate) use crate::pagos::Pagos;
    pub(crate) use crate::views::OrderViewRepository;
    pub(crate) use chrono::{DateTime, Utc};
    pub(crate) use serde::{Deserialize, Serialize};
    pub(crate) use serde_json::{json, Value};
    pub(crate) use sqlx::Row;
    pub(crate) use std::sync::Arc;
    pub(crate) use uuid::Uuid;
}
use self::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LineaDevolucion {
    pub id: String,
    pub oferta_id: String,
    pub sku: String,
    pub cantidad: i64,
    pub precio_unitario_cents: i64,
    pub descuento_cents: i64,
    pub base_cents: i64,
    pub impuesto_cents: i64,
    pub tasa_impuesto_bps: i64,
    pub total_cents: i64,
    pub restock: bool,
    pub ubicacion_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Devolucion {
    pub id: String,
    pub order_id: String,
    pub empresa_id: String,
    pub negocio_id: Option<String>,
    pub motivo: String,
    pub estado: String,
    pub base_cents: i64,
    pub impuesto_cents: i64,
    pub total_cents: i64,
    pub moneda: String,
    pub reembolso_estado: String,
    pub reembolsado_cents: i64,
    pub reembolso_error: String,
    pub nota_credito_id: Option<String>,
    pub nota_credito_estado: String,
    pub nota_credito_error: String,
    pub creado_por: String,
    pub creado_en: String,
    pub actualizado_en: String,
    pub lineas: Vec<LineaDevolucion>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearDevolucionInput {
    #[serde(default)]
    pub motivo: String,
    #[serde(default)]
    pub items: Vec<ItemDevolucionInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ItemDevolucionInput {
    pub oferta_id: String,
    pub cantidad: i64,
    #[serde(default = "restock_por_defecto")]
    pub restock: bool,
}

fn restock_por_defecto() -> bool {
    true
}

pub struct Devoluciones {
    db: Arc<Db>,
    vistas: Arc<OrderViewRepository>,
    clientes: Arc<dyn ClientesPort>,
    pagos: Arc<Pagos>,
    outbox: Outbox,
}

impl Devoluciones {
    pub fn nueva(
        db: Arc<Db>,
        vistas: Arc<OrderViewRepository>,
        clientes: Arc<dyn ClientesPort>,
        pagos: Arc<Pagos>,
        outbox: Outbox,
    ) -> Self {
        Self {
            db,
            vistas,
            clientes,
            pagos,
            outbox,
        }
    }
}
