//! Handlers HTTP de `field-service`.
//!
//! Cada handler replica el controlador Nest: autorización (roles + capacidad),
//! tenant obligatorio, validación DTO y delegación al dominio. El envelope,
//! las métricas y la correlación viven en `http/mod.rs`.

use super::Cuerpo;
use crate::auth::{self, UsuarioContexto, ROLES_CAMPO};
use crate::errors::ApiError;
use crate::field::validacion;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

const ROLES_ADMIN_FLOTA: &[&str] = &["super_admin", "empresario", "admin", "coordinador"];
const ROLES_SUPERVISOR: &[&str] = &[
    "super_admin",
    "empresario",
    "admin",
    "coordinador",
    "supervisor",
];
const ROLES_OPERATIVO: &[&str] = &[
    "super_admin",
    "empresario",
    "admin",
    "coordinador",
    "supervisor",
    "operativo",
];
const CAPACIDAD_RUTAS: &[&str] = &["logistica_rutas"];

async fn autorizar(
    state: &AppState,
    headers: &HeaderMap,
    roles: &[&str],
    capacidades_extra: &[&str],
) -> Result<UsuarioContexto, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(headers)?;
    auth::autorizar(
        &usuario,
        roles,
        capacidades_extra,
        state.capacidades.as_ref(),
    )
    .await?;
    Ok(usuario)
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn sin_cuerpo() -> Result<Response, ApiError> {
    Ok(StatusCode::OK.into_response())
}

fn validar(esquema: &[validacion::ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    validacion::validar(esquema, cuerpo)
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltroRutas {
    personal_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltroPedidos {
    estado: Option<String>,
    ruta_id: Option<String>,
    parte_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltroAsistencia {
    personal_id: Option<String>,
    fecha: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltroIncidencias {
    estado: Option<String>,
    ruta_id: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltroTracking {
    personal_id: Option<String>,
    desde: Option<String>,
    hasta: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FiltroVisitas {
    personal_id: Option<String>,
    fecha: Option<String>,
}

// ── personal ──────────────────────────────────────────────────────────────
pub async fn listar_personal(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(Value::Array(state.field()?.listar_personal(tenant).await?))
}

pub async fn obtener_personal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(state.field()?.obtener_personal(tenant, &id).await?)
}

pub async fn crear_personal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::CREAR_PERSONAL, &cuerpo)?;
    created(state.field()?.crear_personal(tenant, &cuerpo).await?)
}

pub async fn actualizar_personal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::ACTUALIZAR_PERSONAL, &cuerpo)?;
    ok(state
        .field()?
        .actualizar_personal(tenant, &id, &cuerpo)
        .await?)
}

pub async fn eliminar_personal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_ADMIN_FLOTA, &[]).await?;
    let tenant = usuario.tenant()?;
    state.field()?.eliminar_personal(tenant, &id).await?;
    sin_cuerpo()
}

pub async fn ubicacion_personal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(state.field()?.ubicacion_personal(tenant, &id).await?)
}

pub async fn actualizar_ubicacion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::UBICACION, &cuerpo)?;
    ok(state
        .field()?
        .actualizar_ubicacion(tenant, &id, &cuerpo)
        .await?)
}

// ── clientes ──────────────────────────────────────────────────────────────
pub async fn listar_clientes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(Value::Array(state.field()?.listar_clientes(tenant).await?))
}

pub async fn obtener_cliente(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(state.field()?.obtener_cliente(tenant, &id).await?)
}

pub async fn crear_cliente(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::CREAR_CLIENTE, &cuerpo)?;
    created(state.field()?.crear_cliente(tenant, &cuerpo).await?)
}

pub async fn actualizar_cliente(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::ACTUALIZAR_CLIENTE, &cuerpo)?;
    ok(state
        .field()?
        .actualizar_cliente(tenant, &id, &cuerpo)
        .await?)
}

pub async fn eliminar_cliente(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_ADMIN_FLOTA, &[]).await?;
    let tenant = usuario.tenant()?;
    state.field()?.eliminar_cliente(tenant, &id).await?;
    sin_cuerpo()
}

// ── vehículos ─────────────────────────────────────────────────────────────
pub async fn listar_vehiculos(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(Value::Array(state.field()?.listar_vehiculos(tenant).await?))
}

pub async fn obtener_vehiculo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(state.field()?.obtener_vehiculo(tenant, &id).await?)
}

pub async fn crear_vehiculo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_SUPERVISOR, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::CREAR_VEHICULO, &cuerpo)?;
    created(state.field()?.crear_vehiculo(tenant, &cuerpo).await?)
}

pub async fn actualizar_vehiculo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_SUPERVISOR, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::ACTUALIZAR_VEHICULO, &cuerpo)?;
    ok(state
        .field()?
        .actualizar_vehiculo(tenant, &id, &cuerpo)
        .await?)
}

pub async fn eliminar_vehiculo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_ADMIN_FLOTA, &[]).await?;
    let tenant = usuario.tenant()?;
    state.field()?.eliminar_vehiculo(tenant, &id).await?;
    sin_cuerpo()
}

// ── rutas ─────────────────────────────────────────────────────────────────
pub async fn listar_rutas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filtro): Query<FiltroRutas>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, CAPACIDAD_RUTAS).await?;
    let tenant = usuario.tenant()?;
    ok(Value::Array(
        state
            .field()?
            .listar_rutas(tenant, filtro.personal_id.as_deref())
            .await?,
    ))
}

pub async fn obtener_ruta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, CAPACIDAD_RUTAS).await?;
    let tenant = usuario.tenant()?;
    ok(state.field()?.obtener_ruta(tenant, &id).await?)
}

pub async fn crear_ruta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_SUPERVISOR, CAPACIDAD_RUTAS).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::CREAR_RUTA, &cuerpo)?;
    created(state.field()?.crear_ruta(tenant, &cuerpo).await?)
}

pub async fn actualizar_ruta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_SUPERVISOR, CAPACIDAD_RUTAS).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::ACTUALIZAR_RUTA, &cuerpo)?;
    ok(state.field()?.actualizar_ruta(tenant, &id, &cuerpo).await?)
}

pub async fn eliminar_ruta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_ADMIN_FLOTA, CAPACIDAD_RUTAS).await?;
    let tenant = usuario.tenant()?;
    state.field()?.eliminar_ruta(tenant, &id).await?;
    sin_cuerpo()
}

pub async fn agregar_parada(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_SUPERVISOR, CAPACIDAD_RUTAS).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::PARADA, &cuerpo)?;
    created(state.field()?.agregar_parada(tenant, &id, &cuerpo).await?)
}

pub async fn actualizar_parada(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((id, parada_id)): Path<(String, String)>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_SUPERVISOR, CAPACIDAD_RUTAS).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::PARADA, &cuerpo)?;
    ok(state
        .field()?
        .actualizar_parada(tenant, &id, &parada_id, &cuerpo)
        .await?)
}

pub async fn asignar_ruta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_SUPERVISOR, CAPACIDAD_RUTAS).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::ASIGNAR_RUTA, &cuerpo)?;
    created(state.field()?.asignar_ruta(tenant, &id, &cuerpo).await?)
}

// ── pedidos ───────────────────────────────────────────────────────────────
pub async fn listar_pedidos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filtro): Query<FiltroPedidos>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(Value::Array(
        state
            .field()?
            .listar_pedidos(
                tenant,
                filtro.estado.as_deref(),
                filtro.ruta_id.as_deref(),
                filtro.parte_id.as_deref(),
            )
            .await?,
    ))
}

pub async fn obtener_pedido(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(state.field()?.obtener_pedido(tenant, &id).await?)
}

pub async fn crear_pedido(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_OPERATIVO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::CREAR_PEDIDO, &cuerpo)?;
    created(state.field()?.crear_pedido(tenant, &cuerpo).await?)
}

pub async fn actualizar_pedido(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_OPERATIVO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::ACTUALIZAR_PEDIDO, &cuerpo)?;
    ok(state
        .field()?
        .actualizar_pedido(tenant, &id, &cuerpo)
        .await?)
}

pub async fn eliminar_pedido(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_ADMIN_FLOTA, &[]).await?;
    let tenant = usuario.tenant()?;
    state.field()?.eliminar_pedido(tenant, &id).await?;
    sin_cuerpo()
}

pub async fn cambiar_estado_pedido(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_OPERATIVO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::CAMBIAR_ESTADO_PEDIDO, &cuerpo)?;
    ok(state
        .field()?
        .cambiar_estado_pedido(tenant, &id, &cuerpo)
        .await?)
}

// ── asistencia ────────────────────────────────────────────────────────────
pub async fn listar_asistencia(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filtro): Query<FiltroAsistencia>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(Value::Array(
        state
            .field()?
            .listar_asistencia(
                tenant,
                filtro.personal_id.as_deref(),
                filtro.fecha.as_deref(),
            )
            .await?,
    ))
}

pub async fn registrar_asistencia(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::CREAR_ASISTENCIA, &cuerpo)?;
    created(state.field()?.registrar_asistencia(tenant, &cuerpo).await?)
}

// ── incidencias ───────────────────────────────────────────────────────────
pub async fn listar_incidencias(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filtro): Query<FiltroIncidencias>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(Value::Array(
        state
            .field()?
            .listar_incidencias(tenant, filtro.estado.as_deref(), filtro.ruta_id.as_deref())
            .await?,
    ))
}

pub async fn obtener_incidencia(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(state.field()?.obtener_incidencia(tenant, &id).await?)
}

pub async fn crear_incidencia(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::CREAR_INCIDENCIA, &cuerpo)?;
    created(state.field()?.crear_incidencia(tenant, &cuerpo).await?)
}

pub async fn actualizar_incidencia(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::ACTUALIZAR_INCIDENCIA, &cuerpo)?;
    ok(state
        .field()?
        .actualizar_incidencia(tenant, &id, &cuerpo)
        .await?)
}

pub async fn eliminar_incidencia(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_ADMIN_FLOTA, &[]).await?;
    let tenant = usuario.tenant()?;
    state.field()?.eliminar_incidencia(tenant, &id).await?;
    sin_cuerpo()
}

// ── tracking ──────────────────────────────────────────────────────────────
pub async fn listar_tracking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filtro): Query<FiltroTracking>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(Value::Array(
        state
            .field()?
            .listar_tracking(
                tenant,
                filtro.personal_id.as_deref(),
                filtro.desde.as_deref(),
                filtro.hasta.as_deref(),
            )
            .await?,
    ))
}

pub async fn registrar_tracking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::TRACKING_BULK, &cuerpo)?;
    let registros = cuerpo
        .get("registros")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    created(Value::Array(
        state
            .field()?
            .registrar_tracking(tenant, &registros)
            .await?,
    ))
}

// ── visitas ───────────────────────────────────────────────────────────────
pub async fn listar_visitas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filtro): Query<FiltroVisitas>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(Value::Array(
        state
            .field()?
            .listar_visitas(
                tenant,
                filtro.personal_id.as_deref(),
                filtro.fecha.as_deref(),
            )
            .await?,
    ))
}

pub async fn registrar_visita(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::CREAR_VISITA, &cuerpo)?;
    created(state.field()?.registrar_visita(tenant, &cuerpo).await?)
}

// ── cumplimiento y sync ───────────────────────────────────────────────────
pub async fn obtener_cumplimiento(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((ruta_id, fecha)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    ok(state
        .field()?
        .obtener_cumplimiento(tenant, &ruta_id, &fecha)
        .await?)
}

pub async fn guardar_cumplimiento(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_SUPERVISOR, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::GUARDAR_CUMPLIMIENTO, &cuerpo)?;
    created(state.field()?.guardar_cumplimiento(tenant, &cuerpo).await?)
}

pub async fn sincronizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, &ROLES_CAMPO, &[]).await?;
    let tenant = usuario.tenant()?;
    validar(validacion::SYNC_REQUEST, &cuerpo)?;
    let operaciones = cuerpo
        .get("operaciones")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    created(Value::Array(
        state.field()?.sincronizar(tenant, &operaciones).await,
    ))
}

/// Utilidad para tests de handlers: cuerpo de error JSON con el shape público.
#[allow(dead_code)]
pub fn error_json(codigo: &str, mensaje: &str) -> Value {
    json!({ "codigo": codigo, "mensaje": mensaje })
}
