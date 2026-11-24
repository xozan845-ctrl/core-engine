use super::*;

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
