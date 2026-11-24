use super::*;

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
