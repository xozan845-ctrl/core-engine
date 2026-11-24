use super::*;

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
