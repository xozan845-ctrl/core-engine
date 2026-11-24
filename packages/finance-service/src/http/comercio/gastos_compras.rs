use super::*;

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
