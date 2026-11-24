use super::*;

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
