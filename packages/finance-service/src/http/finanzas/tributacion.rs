use super::*;

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

pub async fn jurisdicciones_actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(codigo): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Jurisdiccion>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: ActualizarJurisdiccion = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    Ok(Json(
        state
            .tributacion
            .actualizar_jurisdiccion(&codigo, dto)
            .await?,
    ))
}

pub async fn tasas_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(jurisdiccion): Path<String>,
) -> Result<Json<Vec<TasaImpuesto>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let codigo = jurisdiccion.trim().to_uppercase();
    Ok(Json(state.tributacion.tasas_de(Some(&codigo)).await?))
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GuardarTasasRequest {
    #[serde(default)]
    pub tasas: Vec<CrearTasaImpuesto>,
}

pub async fn tasas_guardar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(jurisdiccion): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Vec<TasaImpuesto>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: GuardarTasasRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    Ok(Json(
        state
            .tributacion
            .guardar_tasas(jurisdiccion.trim(), dto.tasas)
            .await?,
    ))
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
    let usuario = contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let empresa_id = if usuario.es_admin_global() {
        query
            .get("empresa_id")
            .map(String::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string)
    } else {
        Some(usuario.tenant()?.to_string())
    };
    Ok(Json(
        state
            .tributacion
            .declaraciones(FiltrosDeclaracion {
                empresa_id,
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
    pub empresa_id: Option<String>,
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
    let usuario = contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: GenerarDeclaracionesRequest = serde_json::from_value(cuerpo).unwrap_or_default();
    let periodo = match (dto.inicio, dto.fin) {
        (Some(inicio), Some(fin)) => Some(PeriodoMensual { inicio, fin }),
        _ => None,
    };
    let empresa_id = if usuario.es_admin_global() {
        dto.empresa_id
            .as_deref()
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string)
    } else {
        Some(usuario.tenant()?.to_string())
    };
    let mut generadas = state
        .tributacion
        .generar_declaraciones(periodo.clone(), empresa_id.as_deref())
        .await?;
    if empresa_id.is_none() {
        generadas.extend(
            state
                .tributacion
                .generar_declaraciones_tenants(periodo)
                .await?,
        );
    }
    Ok(Json(generadas))
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
