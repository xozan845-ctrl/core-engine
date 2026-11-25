//! Libro de operación de comercio (ADR-66 a ADR-72).
//!
//! Persiste los recursos que el frontend de empresa/vendedor administra
//! (bancos, créditos, gastos, caja, compras, clientes y códigos de barras) en
//! `finance.workspace_registros`, con aislamiento por tenant o por vendedor.

use crate::auth::{UsuarioContexto, ROLES_COMERCIO};
use crate::db::{iso_ms, jsonb, timestamp, uuid_texto, Db};
use crate::errors::{error_db, es_violacion_unicidad, ApiError};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

pub const RECURSOS: [&str; 16] = [
    "bancos-cuentas",
    "creditos",
    "gastos",
    "cuentas-por-pagar",
    "proveedores",
    "compras",
    "ordenes-compra",
    "caja-metodos-pago",
    "caja-movimientos",
    "caja-cierres",
    "caja-chica-fondo",
    "caja-chica-movimientos",
    "caja-transferencias",
    "configuracion-documentos",
    "clientes",
    "codigos-barras",
];

/// Recursos que solo existen en el workspace de empresa (ADR-66/ADR-72).
pub const RECURSOS_SOLO_EMPRESA: [&str; 10] = [
    "gastos",
    "proveedores",
    "compras",
    "ordenes-compra",
    "caja-metodos-pago",
    "caja-movimientos",
    "caja-cierres",
    "caja-chica-fondo",
    "caja-chica-movimientos",
    "caja-transferencias",
];

pub const METODOS_PAGO: [&str; 5] = ["efectivo", "tarjeta", "transferencia", "credito", "otro"];
pub const TIPOS_GASTO: [&str; 3] = ["inmediato", "programado", "recurrente"];
pub const ESTADOS_COMPRA: [&str; 3] = ["recibida", "pendiente", "anulada"];
pub const ORIGENES_COMPRA: [&str; 2] = ["productos", "directa"];
pub const TIPOS_COSTO: [&str; 4] = ["inventario", "servicio", "gasto", "otro"];
pub const SIMBOLOGIAS: [&str; 2] = ["EAN13", "CODE128"];
pub const TIPOS_CUENTA_BANCO: [&str; 3] = ["ahorro", "corriente", "wallet"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ambito {
    pub tenant_id: Option<String>,
    pub propietario_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CuentaCredito {
    pub id: String,
    pub empresa_id: String,
    pub parte_id: String,
    pub parte_nombre: String,
    pub limite_cents: i64,
    pub usado_cents: i64,
    pub disponible_cents: i64,
    pub activo: bool,
    pub actualizado_en: String,
}

/// Registro público: columnas de control + payload del recurso.
pub type Registro = Map<String, Value>;

pub struct Comercio {
    db: Arc<Db>,
}

impl Comercio {
    pub fn db(&self) -> &Arc<Db> {
        &self.db
    }
}

// ── contenedores de petición ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LineaCompra {
    #[serde(default)]
    pub product_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub sku: Option<String>,
    pub quantity: f64,
    pub unit_cost_cents: i64,
    #[serde(default)]
    pub sale_price_cents: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CuentaBanco {
    pub bank: String,
    pub alias: String,
    pub account_number: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Credito {
    pub customer: String,
    pub concept: String,
    #[serde(default)]
    pub date: Option<String>,
    pub due_date: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AbonoCredito {
    pub monto_cents: i64,
    #[serde(default)]
    pub metodo_pago: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CuentaCreditoInput {
    pub limite_cents: i64,
    #[serde(default)]
    pub activo: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreditoInterno {
    pub empresa_id: String,
    pub parte_id: String,
    pub order_id: String,
    pub monto_cents: i64,
    pub concepto: String,
    #[serde(default)]
    pub vendedor_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Gasto {
    pub date: String,
    pub concept: String,
    pub category: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub amount_cents: i64,
    pub payment_method: String,
    #[serde(default)]
    pub includes_vat: Option<bool>,
    #[serde(default)]
    pub ir_retention_cents: Option<i64>,
    #[serde(default)]
    pub beneficiary: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub recurrence: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CuentaPorPagar {
    pub supplier: String,
    pub concept: String,
    pub due_date: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Proveedor {
    pub name: String,
    #[serde(default)]
    pub ruc: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Compra {
    #[serde(default)]
    pub number: Option<String>,
    pub date: String,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    pub payment_method: String,
    pub status: String,
    pub origin: String,
    pub cost_type: String,
    pub lines: Vec<LineaCompra>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub retention_cents: Option<i64>,
    #[serde(default)]
    pub user_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompraDirecta {
    #[serde(default)]
    pub number: Option<String>,
    pub date: String,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    pub concept: String,
    pub cost_type: String,
    pub amount_cents: i64,
    pub payment_method: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub retention_cents: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CompraProductoNuevo {
    #[serde(default)]
    pub number: Option<String>,
    pub date: String,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    pub product_name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub sku: Option<String>,
    pub payment_method: String,
    pub unit_cost_cents: i64,
    pub sale_price_cents: i64,
    pub quantity: f64,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OrdenCompra {
    #[serde(default)]
    pub number: Option<String>,
    pub date: String,
    #[serde(default)]
    pub supplier_id: Option<String>,
    #[serde(default)]
    pub supplier_name: Option<String>,
    pub lines: Vec<LineaCompra>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetodoPago {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MovimientoCaja {
    pub date: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub concept: String,
    pub amount_cents: i64,
    pub payment_method: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub original_amount_cents: Option<i64>,
    #[serde(default)]
    pub exchange_rate: Option<f64>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub observations: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CierreCaja {
    pub date: String,
    pub counted_balance_cents: i64,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FondoCajaChica {
    pub amount_cents: i64,
    pub responsible: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MovimientoCajaChica {
    pub date: String,
    #[serde(rename = "type")]
    pub tipo: String,
    pub concept: String,
    pub amount_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TransferenciaCaja {
    pub date: String,
    pub from_id: String,
    pub from_label: String,
    pub from_kind: String,
    pub to_id: String,
    pub to_label: String,
    pub to_kind: String,
    pub amount_cents: i64,
    pub concept: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConfiguracionDocumentos {
    pub logo_size: String,
    pub header_color: String,
    pub match_table_color: bool,
    pub footer_message: String,
    pub show_ruc: bool,
    pub show_address: bool,
    pub show_phone: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClienteComercio {
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub buyer_id: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportarClientes {
    pub clientes: Vec<ClienteComercio>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodigoBarras {
    pub product_name: String,
    pub sku: String,
    #[serde(default)]
    pub code: Option<String>,
    pub symbology: String,
    pub copies: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportacionClientes {
    pub imported: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PreviewCierre {
    pub date: String,
    pub opening_balance_cents: i64,
    pub income_cents: i64,
    pub expense_cents: i64,
    pub expected_balance_cents: i64,
    pub movements_count: i64,
    pub existing: Option<Registro>,
}

fn normalizar_moneda(valor: &str) -> Result<String, ApiError> {
    let codigo = valor.trim().to_uppercase();
    let valido = codigo.len() == 3 && codigo.chars().all(|c| c.is_ascii_uppercase());
    if !valido {
        return Err(ApiError::validacion(json!([
            {"campo": "moneda", "restricciones": ["matches"]}
        ])));
    }
    Ok(codigo)
}

impl Comercio {
    pub fn nuevo(db: Arc<Db>) -> Self {
        Self { db }
    }
}

mod caja;
mod clientes;
mod creditos;
mod gastos;
mod generico;
mod helpers;
mod tipos_cambio;

pub use helpers::*;

#[cfg(test)]
mod tests;
