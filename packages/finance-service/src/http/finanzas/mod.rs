//! Rutas públicas del dominio financiero: contabilidad, tributación,
//! facturación, modelo (proyecciones/KPIs/tablero) y situación del vendedor.

use crate::auth::{ROLES, ROLES_FINANZAS};
use crate::comercio::numero;
use crate::contabilidad::{DetalleAsiento, ParametrosAsiento};
use crate::errors::ApiError;
use crate::facturacion::{Comprobante, EmitirComprobante, SerieComprobante};
use crate::finanzas::alertas::{OpcionesTablero, Tablero};
use crate::finanzas::kpis::KpisFinancieros;
use crate::finanzas::proyecciones::{
    EstudiosSensibilidad, PlanBienal, Proyeccion, ProyeccionResumen, Proyecciones,
    SupuestosProyeccion,
};
use crate::http::{contexto, fecha_query, query_f64, query_i64, Cuerpo};
use crate::state::AppState;
use crate::tributacion::{
    ActualizarJurisdiccion, CrearJurisdiccion, CrearRegimen, CrearTasaImpuesto, FiltrosDeclaracion,
    Jurisdiccion, PeriodoMensual, RegimenFiscal, RegistrarSujeto, SujetoTributario, TasaImpuesto,
};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use chrono::{Local, NaiveDate};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

const CAP_FACTURACION: &str = "facturacion";

// ── contabilidad ────────────────────────────────────────────────────────────

mod contabilidad;
mod facturacion;
mod proyecciones;
mod tributacion;

pub use contabilidad::*;
pub use facturacion::*;
pub use proyecciones::*;
pub use tributacion::*;

fn validar_supuestos(supuestos: &SupuestosProyeccion) -> Result<(), ApiError> {
    if !(1..=60).contains(&supuestos.horizonte_meses) {
        return Err(ApiError::validacion(json!([
            {"campo": "supuestos.horizonte_meses", "restricciones": ["min:1", "max:60"]}
        ])));
    }
    if supuestos.pedidos_por_vendedor.is_empty() {
        return Err(ApiError::validacion(json!([
            {"campo": "supuestos.pedidos_por_vendedor", "restricciones": ["arrayMinSize"]}
        ])));
    }
    if !(0.0..=1.0).contains(&supuestos.churn_tasa) {
        return Err(ApiError::validacion(json!([
            {"campo": "supuestos.churn_tasa", "restricciones": ["min:0", "max:1"]}
        ])));
    }
    if !(0.0..=1.0).contains(&supuestos.comision_tasa) {
        return Err(ApiError::validacion(json!([
            {"campo": "supuestos.comision_tasa", "restricciones": ["min:0", "max:1"]}
        ])));
    }
    supuestos.validar()
}

fn validar_in(valor: &str, campo: &str, permitidos: &[&str]) -> Result<(), ApiError> {
    if !permitidos.contains(&valor) {
        return Err(ApiError::validacion(json!([
            {"campo": campo, "restricciones": ["isIn"]}
        ])));
    }
    Ok(())
}

fn validacion_body(_: serde_json::Error) -> ApiError {
    ApiError::validacion(json!([
        {"campo": "body", "restricciones": ["isObject"]}
    ]))
}

/// Fecha local usada por el generador mensual (expuesta para las pruebas).
pub fn hoy_local() -> NaiveDate {
    Local::now().date_naive()
}

/// Utilería usada por el planificador y los tests de declaraciones.
pub fn periodo_actual() -> PeriodoMensual {
    crate::tributacion::Tributacion::periodo_del_mes(Local::now().date_naive())
}

/// Utilería compartida: formatea centavos en el contrato de comercio.
pub fn monto_json(valor: Option<&Value>) -> String {
    crate::money::string(numero(valor))
}

#[cfg(test)]
mod tests {
    use super::{hoy_local, periodo_actual, validar_supuestos, SupuestosProyeccion};

    #[test]
    fn debe_validar_los_supuestos_del_modelo() {
        let mut supuestos = SupuestosProyeccion::supuestos_informe();
        validar_supuestos(&supuestos).expect("validos");
        supuestos.comision_tasa = 1.5;
        assert!(validar_supuestos(&supuestos).is_err());
        supuestos.comision_tasa = 0.12;
        supuestos.horizonte_meses = 0;
        assert!(validar_supuestos(&supuestos).is_err());
    }

    #[test]
    fn debe_exponer_el_periodo_actual() {
        let periodo = periodo_actual();
        assert!(periodo.inicio.ends_with("-01"));
        assert!(hoy_local().to_string().starts_with(&periodo.inicio[..7]));
    }
}
