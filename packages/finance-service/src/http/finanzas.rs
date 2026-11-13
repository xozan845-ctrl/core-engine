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
    CrearJurisdiccion, CrearRegimen, FiltrosDeclaracion, Jurisdiccion, PeriodoMensual,
    RegimenFiscal, RegistrarSujeto, SujetoTributario,
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

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearCuentaRequest {
    pub codigo: String,
    pub nombre: String,
    pub tipo: String,
    pub naturaleza: String,
    #[serde(default)]
    pub nivel: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EstadoCuentaRequest {
    pub estado: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DetalleRequest {
    pub cuenta_codigo: String,
    #[serde(default)]
    pub debe_cents: Option<i64>,
    #[serde(default)]
    pub haber_cents: Option<i64>,
    #[serde(default)]
    pub concepto: Option<String>,
    #[serde(default)]
    pub orden: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearAsientoRequest {
    pub concepto: String,
    #[serde(default)]
    pub fecha: Option<String>,
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub referencia_tipo: Option<String>,
    #[serde(default)]
    pub referencia_id: Option<String>,
    #[serde(default)]
    pub detalles: Vec<DetalleRequest>,
}

pub async fn cuentas_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::contabilidad::Cuenta>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(state.contabilidad.plan_de_cuentas().await?))
}

pub async fn cuentas_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<crate::contabilidad::Cuenta>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: CrearCuentaRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    if dto.nombre.trim().len() < 3 {
        return Err(ApiError::validacion(json!([
            {"campo": "nombre", "restricciones": ["minLength:3"]}
        ])));
    }
    validar_in(
        &dto.tipo,
        "tipo",
        &["ACTIVO", "PASIVO", "CAPITAL", "INGRESO", "COSTO", "GASTO"],
    )?;
    validar_in(&dto.naturaleza, "naturaleza", &["DEUDORA", "ACREEDORA"])?;
    let cuenta = state
        .contabilidad
        .crear_cuenta(
            &dto.codigo,
            &dto.nombre,
            &dto.tipo,
            &dto.naturaleza,
            dto.nivel.unwrap_or(3),
        )
        .await?;
    Ok(Json(cuenta))
}

pub async fn cuenta_estado(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(codigo): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<crate::contabilidad::Cuenta>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: EstadoCuentaRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    validar_in(&dto.estado, "estado", &["activa", "inactiva"])?;
    state
        .contabilidad
        .estado_cuenta(&codigo, &dto.estado)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Cuenta", &codigo))
}

pub async fn asientos_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<crate::contabilidad::Asiento>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let desde = fecha_query(query.get("desde"))?;
    let hasta = fecha_query(query.get("hasta"))?;
    let limite = query_i64(&query, "limite", 200);
    Ok(Json(
        state
            .contabilidad
            .libro_diario(desde, hasta, limite)
            .await?,
    ))
}

pub async fn asiento_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<crate::contabilidad::Asiento>, ApiError> {
    let usuario = contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: CrearAsientoRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    if dto.concepto.trim().len() < 3 {
        return Err(ApiError::validacion(json!([
            {"campo": "concepto", "restricciones": ["minLength:3"]}
        ])));
    }
    if dto.detalles.len() < 2 {
        return Err(ApiError::validacion(json!([
            {"campo": "detalles", "restricciones": ["arrayMinSize"]}
        ])));
    }
    let fecha = fecha_query(dto.fecha.as_ref())?;
    let detalles: Vec<DetalleAsiento> = dto
        .detalles
        .into_iter()
        .map(|detalle| DetalleAsiento {
            cuenta_codigo: detalle.cuenta_codigo,
            debe_cents: detalle.debe_cents.unwrap_or(0),
            haber_cents: detalle.haber_cents.unwrap_or(0),
            concepto: detalle.concepto,
            orden: detalle.orden,
        })
        .collect();
    let asiento = state
        .contabilidad
        .registrar(ParametrosAsiento {
            concepto: dto.concepto,
            tipo: dto.tipo.unwrap_or_else(|| "MANUAL".to_string()),
            referencia_tipo: dto.referencia_tipo,
            referencia_id: dto.referencia_id,
            fecha,
            moneda: None,
            creado_por: Some(usuario.user_id),
            detalles,
        })
        .await?;
    Ok(Json(asiento))
}

pub async fn asiento_anular(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::contabilidad::Asiento>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    state
        .contabilidad
        .anular(&id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Asiento", &id))
}

pub async fn libro_mayor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cuenta): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<crate::contabilidad::LibroMayor>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let hasta = fecha_query(query.get("hasta"))?;
    Ok(Json(state.contabilidad.libro_mayor(&cuenta, hasta).await?))
}

pub async fn libro_ventas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<crate::contabilidad::LibroIva>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let desde = fecha_query(query.get("desde"))?;
    let hasta = fecha_query(query.get("hasta"))?;
    Ok(Json(state.contabilidad.libro_ventas(desde, hasta).await?))
}

pub async fn libro_compras(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<crate::contabilidad::LibroIva>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let desde = fecha_query(query.get("desde"))?;
    let hasta = fecha_query(query.get("hasta"))?;
    Ok(Json(state.contabilidad.libro_compras(desde, hasta).await?))
}

// ── tributación ─────────────────────────────────────────────────────────────

pub async fn jurisdicciones_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Jurisdiccion>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(state.tributacion.listar_jurisdicciones().await?))
}

pub async fn jurisdicciones_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Jurisdiccion>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: CrearJurisdiccion = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    Ok(Json(state.tributacion.crear_jurisdiccion(dto).await?))
}

pub async fn regimenes_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<RegimenFiscal>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(
        state
            .tributacion
            .regimenes_de(query.get("jurisdiccion").map(String::as_str))
            .await?,
    ))
}

pub async fn regimenes_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<RegimenFiscal>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: CrearRegimen = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    let codigo = dto.codigo.clone();
    state
        .tributacion
        .crear_regimen(dto)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Regimen", &codigo))
}

pub async fn sujetos_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SujetoTributario>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(state.tributacion.sujetos().await?))
}

pub async fn sujetos_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<SujetoTributario>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: RegistrarSujeto = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    let id = dto.id.clone();
    state
        .tributacion
        .registrar_sujeto(dto)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Sujeto", &id))
}

pub async fn sujetos_baja(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<SujetoTributario>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    state
        .tributacion
        .dar_de_baja(&id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Sujeto", &id))
}

pub async fn declaraciones_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<crate::tributacion::Declaracion>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(
        state
            .tributacion
            .declaraciones(FiltrosDeclaracion {
                tipo: query.get("tipo").cloned(),
                periodo_inicio: query.get("periodo_inicio").cloned(),
                estado: query.get("estado").cloned(),
            })
            .await?,
    ))
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GenerarDeclaracionesRequest {
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub inicio: Option<String>,
    #[serde(default)]
    pub fin: Option<String>,
}

pub async fn declaraciones_generar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Vec<crate::tributacion::Declaracion>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: GenerarDeclaracionesRequest = serde_json::from_value(cuerpo).unwrap_or_default();
    let periodo = match (dto.inicio, dto.fin) {
        (Some(inicio), Some(fin)) => Some(PeriodoMensual { inicio, fin }),
        _ => None,
    };
    Ok(Json(
        state.tributacion.generar_declaraciones(periodo).await?,
    ))
}

pub async fn declaracion_presentar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::tributacion::Declaracion>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    state
        .tributacion
        .presentar(&id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Declaracion", &id))
}

pub async fn declaracion_pagar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::tributacion::Declaracion>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    state
        .tributacion
        .marcar_pagada(&id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Declaracion", &id))
}

// ── facturación ─────────────────────────────────────────────────────────────

pub async fn comprobantes_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Comprobante>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let desde = fecha_query(query.get("desde"))?;
    let hasta = fecha_query(query.get("hasta"))?;
    Ok(Json(
        state
            .facturacion
            .listar(desde, hasta, query.get("estado").map(String::as_str))
            .await?,
    ))
}

pub async fn comprobantes_series(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<SerieComprobante>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(state.facturacion.series().await?))
}

pub async fn comprobantes_emitir(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Comprobante>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: EmitirComprobante = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    Ok(Json(state.facturacion.emitir(dto).await?))
}

pub async fn comprobante_anular(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Comprobante>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    state
        .facturacion
        .anular(&id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Comprobante", &id))
}

// ── modelo financiero ───────────────────────────────────────────────────────

pub async fn proyecciones_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Proyeccion>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: CrearProyeccionRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    if dto.nombre.trim().len() < 3 {
        return Err(ApiError::validacion(json!([
            {"campo": "nombre", "restricciones": ["minLength:3"]}
        ])));
    }
    validar_supuestos(&dto.supuestos)?;
    Ok(Json(
        state.proyecciones.crear(&dto.nombre, dto.supuestos).await?,
    ))
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrearProyeccionRequest {
    pub nombre: String,
    pub supuestos: SupuestosProyeccion,
}

pub async fn proyecciones_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProyeccionResumen>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(state.proyecciones.listar().await?))
}

pub async fn proyeccion_detalle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Proyeccion>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    state
        .proyecciones
        .obtener(&id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Proyeccion", &id))
}

pub async fn proyecciones_sensibilidad(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EstudiosSensibilidad>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(Proyecciones::estudios(
        &SupuestosProyeccion::supuestos_informe(),
    )?))
}

pub async fn proyecciones_plan_bienal(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PlanBienal>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(Proyecciones::plan_bienal(
        &SupuestosProyeccion::supuestos_informe(),
    )?))
}

pub async fn punto_equilibrio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let horizonte = query_i64(&query, "horizonte_meses", 12).clamp(1, 60);
    let supuestos = SupuestosProyeccion {
        horizonte_meses: horizonte,
        vendedores_iniciales: 1,
        entrada_vendedores_mes: 0,
        churn_tasa: 0.0,
        pedidos_por_vendedor: (0..horizonte)
            .map(|indice| (25.0 + indice as f64 * 1.6).min(42.0))
            .collect(),
        ticket_promedio_cents: query_i64(&query, "ticket_promedio_cents", 45_000),
        comision_tasa: query_f64(&query, "comision_tasa", 0.12),
        costos_fijos_cents: query_i64(&query, "costos_fijos_cents", 300_000),
        costos_fijos_desde_mes7_cents: None,
        inversion_inicial_cents: 0,
        tasa_descuento_mensual: None,
    };
    let (filas, _) = Proyecciones::calcular(&supuestos)?;
    let indicadores = Proyecciones::indicadores(&filas, &supuestos);
    Ok(Json(json!({
        "punto_equilibrio_caja_cents": indicadores.punto_equilibrio_caja_cents,
        "pedidos_equilibrio_caja": indicadores.pedidos_equilibrio_caja,
        "escenarios_equilibrio": indicadores.escenarios_equilibrio,
        "margen_seguridad_por_mes": indicadores.margen_seguridad_por_mes,
    })))
}

pub async fn kpis(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<KpisFinancieros>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(
        state
            .kpis
            .kpis(query.get("mes").map(String::as_str))
            .await?,
    ))
}

pub async fn tablero(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Tablero>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let costo = query
        .get("costo_entrega_por_pedido_cents")
        .and_then(|valor| valor.parse::<i64>().ok());
    Ok(Json(
        state
            .alertas
            .tablero(OpcionesTablero {
                mes: query.get("mes").cloned(),
                costo_entrega_por_pedido_cents: costo,
            })
            .await?,
    ))
}

// ── vendedor fiscal ─────────────────────────────────────────────────────────

pub async fn vendedor_fiscal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let usuario = contexto(&state, &headers, &[ROLES[3]], &[], &[]).await?;
    let sujeto = state.tributacion.sujeto_fiscal_de(&usuario.user_id).await?;
    let resumen = state
        .kpis
        .resumen_del_vendedor(
            state.contabilidad.db(),
            &usuario.user_id,
            query.get("mes").map(String::as_str),
        )
        .await?;
    Ok(Json(json!({
        "sujeto_fiscal": sujeto,
        "resumen_comisiones": resumen,
        "nota": "Comisiones documentadas para la declaracion del vendedor ante la DGI (Ley 822 / regimen de Cuota Fija).",
    })))
}

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
    use super::*;

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
