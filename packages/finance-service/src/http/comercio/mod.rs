//! Rutas públicas de la operación de comercio (ADR-66 a ADR-72).

mod bancos_creditos;
mod caja;
mod clientes;
mod eliminar;
mod gastos_compras;
mod tipos_cambio;

pub use bancos_creditos::*;
pub use caja::*;
pub use clientes::*;
pub use eliminar::*;
pub use gastos_compras::*;
pub use tipos_cambio::*;

use crate::auth::ROLES_COMERCIO;
use crate::comercio::{
    AbonoCredito, CierreCaja, ClienteComercio, CodigoBarras, Compra, CompraDirecta,
    CompraProductoNuevo, ConfiguracionDocumentos, Credito, CuentaBanco, CuentaCredito,
    CuentaCreditoInput, CuentaPorPagar, FondoCajaChica, Gasto, ImportacionClientes,
    ImportarClientes, MetodoPago, MovimientoCaja, MovimientoCajaChica, OrdenCompra, PreviewCierre,
    Proveedor, Registro, TransferenciaCaja,
};
use crate::errors::ApiError;
use crate::http::{contexto, Cuerpo};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

async fn auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<crate::auth::UsuarioContexto, ApiError> {
    contexto(state, headers, &ROLES_COMERCIO, &[], &[]).await
}

fn body<T: serde::de::DeserializeOwned>(cuerpo: Value) -> Result<T, ApiError> {
    serde_json::from_value(cuerpo).map_err(|_| {
        ApiError::validacion(serde_json::json!([
            {"campo": "body", "restricciones": ["isObject"]}
        ]))
    })
}

// ── bancos ──────────────────────────────────────────────────────────────────
