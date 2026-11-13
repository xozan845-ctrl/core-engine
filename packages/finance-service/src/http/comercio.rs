//! Rutas públicas de la operación de comercio (ADR-66 a ADR-72).

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

pub async fn bancos_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .listar("bancos-cuentas", &usuario, None)
            .await?,
    ))
}

pub async fn bancos_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: CuentaBanco = body(cuerpo)?;
    Ok(Json(state.comercio.crear_banco(dto, &usuario).await?))
}

pub async fn bancos_actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .actualizar("bancos-cuentas", &id, cuerpo, &usuario)
            .await?,
    ))
}

// ── créditos ────────────────────────────────────────────────────────────────

pub async fn creditos_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state.comercio.listar("creditos", &usuario, None).await?,
    ))
}

pub async fn creditos_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: Credito = body(cuerpo)?;
    Ok(Json(state.comercio.crear_credito(dto, &usuario).await?))
}

pub async fn creditos_pagar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(state.comercio.pagar_credito(&id, &usuario).await?))
}

pub async fn creditos_abonar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: AbonoCredito = body(cuerpo)?;
    Ok(Json(
        state.comercio.abonar_credito(&id, dto, &usuario).await?,
    ))
}

pub async fn cuentas_credito_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<CuentaCredito>>, ApiError> {
    let usuario = contexto(&state, &headers, &ROLES_COMERCIO, &[], &["ventas_credito"]).await?;
    Ok(Json(
        state
            .comercio
            .listar_cuentas_credito(&usuario, query.get("empresa_id").map(String::as_str))
            .await?,
    ))
}

pub async fn cuentas_credito_guardar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(parte_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<CuentaCredito>, ApiError> {
    let usuario = contexto(&state, &headers, &ROLES_COMERCIO, &[], &["ventas_credito"]).await?;
    let dto: CuentaCreditoInput = body(cuerpo)?;
    Ok(Json(
        state
            .comercio
            .guardar_cuenta_credito(
                &parte_id,
                dto,
                &usuario,
                query.get("empresa_id").map(String::as_str),
            )
            .await?,
    ))
}

// ── gastos ──────────────────────────────────────────────────────────────────

pub async fn gastos_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(state.comercio.listar("gastos", &usuario, None).await?))
}

pub async fn gastos_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: Gasto = body(cuerpo)?;
    Ok(Json(state.comercio.crear_gasto(dto, &usuario).await?))
}

pub async fn gastos_actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .actualizar("gastos", &id, cuerpo, &usuario)
            .await?,
    ))
}

// ── cuentas por pagar ───────────────────────────────────────────────────────

pub async fn cuentas_por_pagar_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .listar("cuentas-por-pagar", &usuario, None)
            .await?,
    ))
}

pub async fn cuentas_por_pagar_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: CuentaPorPagar = body(cuerpo)?;
    Ok(Json(
        state.comercio.crear_cuenta_por_pagar(dto, &usuario).await?,
    ))
}

// ── proveedores ─────────────────────────────────────────────────────────────

pub async fn proveedores_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state.comercio.listar("proveedores", &usuario, None).await?,
    ))
}

pub async fn proveedores_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: Proveedor = body(cuerpo)?;
    Ok(Json(state.comercio.crear_proveedor(dto, &usuario).await?))
}

// ── compras ─────────────────────────────────────────────────────────────────

pub async fn compras_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state.comercio.listar("compras", &usuario, None).await?,
    ))
}

pub async fn compras_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: Compra = body(cuerpo)?;
    Ok(Json(state.comercio.crear_compra(dto, &usuario).await?))
}

pub async fn compras_directas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: CompraDirecta = body(cuerpo)?;
    Ok(Json(
        state.comercio.crear_compra_directa(dto, &usuario).await?,
    ))
}

pub async fn compras_producto_nuevo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: CompraProductoNuevo = body(cuerpo)?;
    Ok(Json(
        state
            .comercio
            .crear_compra_producto_nuevo(dto, &usuario)
            .await?,
    ))
}

// ── órdenes de compra ───────────────────────────────────────────────────────

pub async fn ordenes_compra_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .listar("ordenes-compra", &usuario, None)
            .await?,
    ))
}

pub async fn ordenes_compra_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: OrdenCompra = body(cuerpo)?;
    Ok(Json(
        state.comercio.crear_orden_compra(dto, &usuario).await?,
    ))
}

pub async fn ordenes_compra_convertir(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state.comercio.convertir_orden_compra(&id, &usuario).await?,
    ))
}

// ── configuración de documentos ─────────────────────────────────────────────

pub async fn configuracion_documentos_obtener(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .obtener_configuracion_documentos(&usuario)
            .await?,
    ))
}

pub async fn configuracion_documentos_guardar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: ConfiguracionDocumentos = body(cuerpo)?;
    Ok(Json(
        state
            .comercio
            .actualizar_configuracion_documentos(dto, &usuario)
            .await?,
    ))
}

// ── caja ────────────────────────────────────────────────────────────────────

pub async fn metodos_pago_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .listar("caja-metodos-pago", &usuario, None)
            .await?,
    ))
}

pub async fn metodos_pago_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: MetodoPago = body(cuerpo)?;
    Ok(Json(
        state
            .comercio
            .crear("caja-metodos-pago", serde_json::json!(dto), &usuario)
            .await?,
    ))
}

pub async fn metodos_pago_actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .actualizar("caja-metodos-pago", &id, cuerpo, &usuario)
            .await?,
    ))
}

pub async fn movimientos_caja_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .listar("caja-movimientos", &usuario, None)
            .await?,
    ))
}

pub async fn movimientos_caja_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: MovimientoCaja = body(cuerpo)?;
    Ok(Json(
        state
            .comercio
            .crear("caja-movimientos", serde_json::json!(dto), &usuario)
            .await?,
    ))
}

pub async fn cierres_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .listar("caja-cierres", &usuario, None)
            .await?,
    ))
}

pub async fn cierres_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<PreviewCierre>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .preview_cierre(&usuario, query.get("fecha").map(String::as_str))
            .await?,
    ))
}

pub async fn cierres_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: CierreCaja = body(cuerpo)?;
    Ok(Json(state.comercio.crear_cierre(dto, &usuario).await?))
}

pub async fn caja_chica_fondo_obtener(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state.comercio.obtener_fondo_caja_chica(&usuario).await?,
    ))
}

pub async fn caja_chica_fondo_guardar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: FondoCajaChica = body(cuerpo)?;
    Ok(Json(
        state
            .comercio
            .guardar_fondo_caja_chica(dto, &usuario)
            .await?,
    ))
}

pub async fn caja_chica_movimientos_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .listar("caja-chica-movimientos", &usuario, None)
            .await?,
    ))
}

pub async fn caja_chica_movimientos_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: MovimientoCajaChica = body(cuerpo)?;
    Ok(Json(
        state
            .comercio
            .crear("caja-chica-movimientos", serde_json::json!(dto), &usuario)
            .await?,
    ))
}

pub async fn transferencias_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .listar("caja-transferencias", &usuario, None)
            .await?,
    ))
}

pub async fn transferencias_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: TransferenciaCaja = body(cuerpo)?;
    Ok(Json(
        state.comercio.crear_transferencia(dto, &usuario).await?,
    ))
}

pub async fn transferencias_anular(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state.comercio.anular_transferencia(&id, &usuario).await?,
    ))
}

// ── clientes ────────────────────────────────────────────────────────────────

pub async fn clientes_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state.comercio.listar("clientes", &usuario, None).await?,
    ))
}

pub async fn clientes_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: ClienteComercio = body(cuerpo)?;
    Ok(Json(state.comercio.crear_cliente(dto, &usuario).await?))
}

pub async fn clientes_importar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<ImportacionClientes>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: ImportarClientes = body(cuerpo)?;
    Ok(Json(state.comercio.importar_clientes(dto, &usuario).await?))
}

pub async fn clientes_actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .actualizar("clientes", &id, cuerpo, &usuario)
            .await?,
    ))
}

// ── códigos de barras ───────────────────────────────────────────────────────

pub async fn codigos_barras_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Registro>>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .listar("codigos-barras", &usuario, None)
            .await?,
    ))
}

pub async fn codigos_barras_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    let dto: CodigoBarras = body(cuerpo)?;
    Ok(Json(
        state.comercio.crear_codigo_barras(dto, &usuario).await?,
    ))
}

// ── eliminaciones genéricas ─────────────────────────────────────────────────

pub async fn eliminar(
    state: AppState,
    headers: HeaderMap,
    recurso: &str,
    id: String,
) -> Result<StatusCode, ApiError> {
    let usuario = auth(&state, &headers).await?;
    state.comercio.eliminar(recurso, &id, &usuario).await?;
    Ok(StatusCode::OK)
}

pub async fn bancos_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    eliminar(state, headers, "bancos-cuentas", id).await
}

pub async fn creditos_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    eliminar(state, headers, "creditos", id).await
}

pub async fn gastos_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    eliminar(state, headers, "gastos", id).await
}

pub async fn cuentas_por_pagar_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    eliminar(state, headers, "cuentas-por-pagar", id).await
}

pub async fn proveedores_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    eliminar(state, headers, "proveedores", id).await
}

pub async fn compras_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    eliminar(state, headers, "compras", id).await
}

pub async fn metodos_pago_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let usuario = auth(&state, &headers).await?;
    state.comercio.eliminar_metodo_pago(&id, &usuario).await?;
    Ok(StatusCode::OK)
}

pub async fn movimientos_caja_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    eliminar(state, headers, "caja-movimientos", id).await
}

pub async fn clientes_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    eliminar(state, headers, "clientes", id).await
}

pub async fn codigos_barras_eliminar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    eliminar(state, headers, "codigos-barras", id).await
}

// ── acciones genéricas sobre el libro ───────────────────────────────────────

pub async fn actualizar_recurso(
    state: AppState,
    headers: HeaderMap,
    recurso: &str,
    id: String,
    cuerpo: Value,
) -> Result<Json<Registro>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    Ok(Json(
        state
            .comercio
            .actualizar(recurso, &id, cuerpo, &usuario)
            .await?,
    ))
}

pub async fn compras_anular(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Registro>, ApiError> {
    actualizar_recurso(
        state,
        headers,
        "compras",
        id,
        serde_json::json!({"status": "anulada"}),
    )
    .await
}

pub async fn ordenes_compra_anular(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Registro>, ApiError> {
    actualizar_recurso(
        state,
        headers,
        "ordenes-compra",
        id,
        serde_json::json!({"status": "anulada"}),
    )
    .await
}

pub async fn cuentas_por_pagar_pagar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Registro>, ApiError> {
    actualizar_recurso(
        state,
        headers,
        "cuentas-por-pagar",
        id,
        serde_json::json!({"status": "pagada", "paidAt": crate::comercio::hoy()}),
    )
    .await
}

pub async fn gasto_pagar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Registro>, ApiError> {
    actualizar_recurso(
        state,
        headers,
        "gastos",
        id,
        serde_json::json!({"status": "pagado"}),
    )
    .await
}

pub async fn gasto_cancelar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Registro>, ApiError> {
    actualizar_recurso(
        state,
        headers,
        "gastos",
        id,
        serde_json::json!({"status": "cancelado"}),
    )
    .await
}

pub async fn movimiento_caja_anular(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Registro>, ApiError> {
    actualizar_recurso(
        state,
        headers,
        "caja-movimientos",
        id,
        serde_json::json!({"status": "anulado"}),
    )
    .await
}
