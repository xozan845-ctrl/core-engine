//! Modelos de la dispersión diaria.

use crate::db::{fecha, timestamp, uuid_opcional, uuid_texto};
use serde::Serialize;
use sqlx::postgres::PgRow;
use sqlx::Row;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfiguracionDispersion {
    pub proveedor_codigo: String,
    pub activo: bool,
    pub hora_local: String,
    pub minimo_cents: i64,
    pub moneda: String,
    pub retencion_bps: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CuentaPago {
    pub id: String,
    pub beneficiario_tipo: String,
    pub beneficiario_id: String,
    pub proveedor: String,
    pub referencia_externa: String,
    pub moneda: String,
    pub activo: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Payout {
    pub id: String,
    pub fecha: String,
    pub beneficiario_tipo: String,
    pub beneficiario_id: String,
    pub empresa_id: Option<String>,
    pub negocio_id: Option<String>,
    pub monto_cents: i64,
    pub retencion_cents: i64,
    pub monto_neto_cents: i64,
    pub moneda: String,
    pub proveedor: String,
    pub estado: String,
    pub referencia_externa: String,
    pub error: String,
    pub creado_en: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PayoutItem {
    pub liquidacion_tipo: String,
    pub liquidacion_id: String,
    pub monto_cents: i64,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct ResumenDia {
    pub liquidaciones_cerradas: u64,
    pub payouts_creados: u64,
    pub payouts_enviados: u64,
    pub payouts_fallidos: u64,
}

/// Adaptador de dispersión. La plataforma puede elegir Tilopay o Pagadito.
impl ConfiguracionDispersion {
    pub(super) fn desde_fila(fila: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            proveedor_codigo: fila.try_get("proveedor_codigo")?,
            activo: fila.try_get("activo")?,
            hora_local: fila.try_get("hora_local")?,
            minimo_cents: i64::from(fila.try_get::<i32, _>("minimo_cents")?),
            moneda: fila.try_get("moneda")?,
            retencion_bps: i64::from(fila.try_get::<i32, _>("retencion_bps")?),
        })
    }
}

impl CuentaPago {
    pub(super) fn desde_fila(fila: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: uuid_texto(fila, "id")?,
            beneficiario_tipo: fila.try_get("beneficiario_tipo")?,
            beneficiario_id: uuid_texto(fila, "beneficiario_id")?,
            proveedor: fila.try_get("proveedor")?,
            referencia_externa: fila.try_get("referencia_externa")?,
            moneda: fila.try_get("moneda")?,
            activo: fila.try_get("activo")?,
        })
    }
}

impl Payout {
    pub(super) fn desde_fila(fila: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: uuid_texto(fila, "id")?,
            fecha: fecha(fila, "fecha")?,
            beneficiario_tipo: fila.try_get("beneficiario_tipo")?,
            beneficiario_id: uuid_texto(fila, "beneficiario_id")?,
            empresa_id: uuid_opcional(fila, "empresa_id")?,
            negocio_id: uuid_opcional(fila, "negocio_id")?,
            monto_cents: i64::from(fila.try_get::<i32, _>("monto_cents")?),
            retencion_cents: i64::from(fila.try_get::<i32, _>("retencion_cents")?),
            monto_neto_cents: i64::from(fila.try_get::<i32, _>("monto_neto_cents")?),
            moneda: fila.try_get("moneda")?,
            proveedor: fila.try_get("proveedor")?,
            estado: fila.try_get("estado")?,
            referencia_externa: fila.try_get("referencia_externa")?,
            error: fila.try_get("error")?,
            creado_en: timestamp(fila, "creado_en")?,
        })
    }
}
