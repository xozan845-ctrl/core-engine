//! Endpoints internos (servicio → servicio) protegidos por clave interna.

use super::{created, ok, query};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn rutas() -> Router<AppState> {
    Router::new()
        .route("/internal/orders", get(listar))
        .route("/internal/orders/{id}", get(orden))
        .route("/internal/orders/{id}/transicion", post(transicion))
        .route("/internal/orders/{id}/reproyectar", post(reproyectar))
        .route("/internal/orders/{id}/historia", get(historia))
        .route("/internal/orders/tenant/purgar", post(purgar_tenant))
        .route("/internal/orders/tenant/exportar", post(exportar_tenant))
}

#[derive(Debug, Deserialize)]
struct PurgaTenant {
    empresa_id: String,
    #[serde(default)]
    negocio_ids: Vec<String>,
    #[serde(default)]
    usuario_ids: Vec<String>,
}

fn purga_error(error: sqlx::Error) -> ApiError {
    crate::logs::error(
        "orders.purga",
        crate::campos! { "msg": "purga fallida", "err": error.to_string() },
    );
    ApiError::interno()
}

/// POST /internal/orders/tenant/exportar — datos del tenant en orders.
async fn exportar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(dto): axum::Json<PurgaTenant>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    let empresa = uuid::Uuid::parse_str(&dto.empresa_id)
        .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser UUID."))?;
    let negocios: Vec<uuid::Uuid> = dto
        .negocio_ids
        .iter()
        .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
        .collect();
    let usuarios: Vec<uuid::Uuid> = dto
        .usuario_ids
        .iter()
        .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
        .collect();
    let pool = state.db()?.pool();

    let ordenes: Value = sqlx::query_scalar(
        "SELECT COALESCE(jsonb_agg(to_jsonb(o) ORDER BY o.creado_en), '[]'::jsonb)
           FROM orders.orden_vista o
          WHERE o.negocio_id = ANY($1::uuid[])
             OR o.vendedor_id = ANY($2::uuid[])
             OR o.comprador_id = ANY($2::uuid[])",
    )
    .bind(&negocios)
    .bind(&usuarios)
    .fetch_one(pool)
    .await
    .map_err(error_interno)?;

    let mut tablas = serde_json::Map::new();
    tablas.insert("ordenes".into(), ordenes);
    for (nombre, tabla) in [
        ("pagos", "orders.pagos"),
        ("devoluciones", "orders.devoluciones"),
        ("entregas_digitales", "orders.entregas_digitales"),
        ("fidelizacion_programas", "orders.fidelizacion_programas"),
        (
            "fidelizacion_recompensas",
            "orders.fidelizacion_recompensas",
        ),
        ("fidelizacion_tarjetas", "orders.fidelizacion_tarjetas"),
        (
            "fidelizacion_movimientos",
            "orders.fidelizacion_movimientos",
        ),
        ("fidelizacion_canjes", "orders.fidelizacion_canjes"),
        ("suscripciones", "orders.suscripciones"),
        ("contratos", "orders.contratos"),
        ("comandas", "orders.comandas"),
        ("mesas", "orders.mesas"),
        ("configuracion_pagos", "orders.configuracion_pagos"),
    ] {
        let datos: Value = sqlx::query_scalar(&format!(
            "SELECT COALESCE(jsonb_agg(to_jsonb(t) ORDER BY t.creado_en), '[]'::jsonb)
               FROM {tabla} t WHERE t.empresa_id = $1"
        ))
        .bind(empresa)
        .fetch_one(pool)
        .await
        .map_err(error_interno)?;
        tablas.insert(nombre.to_string(), datos);
    }
    super::ok(json!({
        "servicio": "orders",
        "estado": "completado",
        "datos": tablas,
    }))
}

/// POST /internal/orders/tenant/purgar — anonimiza datos personales del tenant y
/// elimina datos efímeros. Conserva órdenes y pagos por retención contable.
async fn purgar_tenant(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::Json(dto): axum::Json<PurgaTenant>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    let empresa = uuid::Uuid::parse_str(&dto.empresa_id)
        .map_err(|_| ApiError::dominio("EMPRESA_INVALIDA", "empresa_id debe ser UUID."))?;
    let negocios: Vec<uuid::Uuid> = dto
        .negocio_ids
        .iter()
        .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
        .collect();
    let usuarios: Vec<uuid::Uuid> = dto
        .usuario_ids
        .iter()
        .filter_map(|valor| uuid::Uuid::parse_str(valor).ok())
        .collect();
    let db = state.db()?;
    let mut tx = db
        .transaccion_servicio()
        .await
        .map_err(|error| purga_error(error))?;

    sqlx::query("DELETE FROM orders.carritos WHERE comprador_id = ANY($1::uuid[])")
        .bind(&usuarios)
        .execute(&mut *tx)
        .await
        .map_err(purga_error)?;

    for tabla in [
        "orders.fidelizacion_canjes",
        "orders.fidelizacion_movimientos",
        "orders.fidelizacion_tarjetas",
        "orders.fidelizacion_recompensas",
        "orders.fidelizacion_programas",
        "orders.suscripcion_ejecuciones",
        "orders.comandas",
        "orders.mesas",
        "orders.configuracion_pagos",
    ] {
        sqlx::query(&format!("DELETE FROM {tabla} WHERE empresa_id = $1"))
            .bind(empresa)
            .execute(&mut *tx)
            .await
            .map_err(purga_error)?;
    }

    sqlx::query(
        "UPDATE orders.suscripciones
            SET estado = 'cancelada', comprador_id = NULL, cliente_parte_id = NULL,
                metadata = '{}'::jsonb, actualizado_en = NOW()
          WHERE empresa_id = $1",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await
    .map_err(purga_error)?;

    sqlx::query(
        "UPDATE orders.contratos
            SET titulo = 'Contrato eliminado', descripcion = '', parte_id = NULL,
                metadata = '{}'::jsonb, actualizado_en = NOW()
          WHERE empresa_id = $1",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await
    .map_err(purga_error)?;
    sqlx::query(
        "UPDATE orders.contrato_eventos SET motivo = '', creado_por = NULL WHERE empresa_id = $1",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await
    .map_err(purga_error)?;
    sqlx::query(
        "UPDATE orders.devoluciones SET motivo = '[eliminado]', creado_por = NULL WHERE empresa_id = $1",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await
    .map_err(error_interno)?;
    sqlx::query(
        "UPDATE orders.pagos SET referencia = '', confirmado_por = NULL WHERE empresa_id = $1",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await
    .map_err(purga_error)?;
    sqlx::query(
        "UPDATE orders.entregas_digitales
            SET recurso_url = '', token = gen_random_uuid(), estado = 'revocado',
                comprador_id = NULL, cliente_parte_id = NULL
          WHERE empresa_id = $1",
    )
    .bind(empresa)
    .execute(&mut *tx)
    .await
    .map_err(purga_error)?;

    let anonimizadas = sqlx::query(
        "UPDATE orders.orden_vista
            SET direccion_envio = '{}'::jsonb,
                comprador_id = '00000000-0000-0000-0000-000000000000'::uuid,
                vendedor_id = '00000000-0000-0000-0000-000000000000'::uuid,
                parte_id = NULL
          WHERE negocio_id = ANY($1::uuid[])
             OR vendedor_id = ANY($2::uuid[])
             OR comprador_id = ANY($2::uuid[])",
    )
    .bind(&negocios)
    .bind(&usuarios)
    .execute(&mut *tx)
    .await
    .map_err(purga_error)?
    .rows_affected();

    sqlx::query(
        "UPDATE orders.orden_timeline
            SET payload = jsonb_build_object('redactado', true)
          WHERE order_id IN (
              SELECT id FROM orders.orden_vista
               WHERE negocio_id = ANY($1::uuid[])
                  OR vendedor_id = '00000000-0000-0000-0000-000000000000'::uuid
          )",
    )
    .bind(&dto.negocio_ids)
    .execute(&mut *tx)
    .await
    .map_err(purga_error)?;

    tx.commit().await.map_err(purga_error)?;
    super::ok(json!({
        "servicio": "orders",
        "estado": "completado",
        "detalle": { "ordenes_anonimizadas": anonimizadas },
    }))
}

async fn transicion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    let Some(estado) = query(&params, "estado") else {
        return Err(ApiError::no_encontrado("Estado", "requerido"));
    };
    let vista = state
        .checkout()?
        .transicionar(
            &id,
            estado,
            query(&params, "motivo"),
            query(&params, "actor").or(Some("logistica")),
            None,
        )
        .await?;
    created(vista)
}

async fn orden(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    let orden = state
        .vistas()?
        .encontrar(state.db()?, &id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Orden", &id))?;
    ok(orden)
}

async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    ok(state
        .vistas()?
        .listar_todo(state.db()?, query(&params, "estado"), None)
        .await?)
}

async fn reproyectar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    created(state.checkout()?.reproyectar(&id).await?)
}

async fn historia(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    super::exigir_interna(&headers, &state)?;
    let historia = crate::event_store::OrderEventStore
        .historia_de(state.db()?, &id)
        .await?;
    if historia.is_empty() {
        return Err(ApiError::no_encontrado("Orden", &id));
    }
    let salida: Vec<serde_json::Value> = historia
        .into_iter()
        .map(|evento| {
            json!({
                "event_id": evento.event_id,
                "aggregate_id": evento.aggregate_id,
                "tipo": evento.tipo,
                "payload": evento.payload,
                "version": evento.version,
                "creado_en": evento.creado_en,
            })
        })
        .collect();
    ok(salida)
}

fn error_interno(_: sqlx::Error) -> ApiError {
    ApiError::interno()
}
