use super::*;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearSerieRequest {
    pub serie: String,
    pub tipo: String,
    pub prefijo: String,
    pub ruc_emisor: String,
    pub jurisdiccion: String,
    #[serde(default)]
    pub empresa_id: Option<String>,
    #[serde(default)]
    pub negocio_id: Option<String>,
    #[serde(default)]
    pub activa: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ActualizarSerieRequest {
    #[serde(default)]
    pub activa: Option<bool>,
    #[serde(default)]
    pub prefijo: Option<String>,
}

pub async fn series_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<crate::facturacion::SerieComprobante>>, ApiError> {
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
            .facturacion
            .series_de(
                query.get("negocio_id").map(String::as_str),
                empresa_id.as_deref(),
            )
            .await?,
    ))
}

pub async fn series_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<crate::facturacion::SerieComprobante>, ApiError> {
    let usuario = contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: CrearSerieRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    let empresa_id = if usuario.es_admin_global() {
        dto.empresa_id
    } else {
        Some(usuario.tenant()?.to_string())
    };
    Ok(Json(
        state
            .facturacion
            .crear_serie(
                &dto.serie,
                &dto.tipo,
                &dto.prefijo,
                &dto.ruc_emisor,
                &dto.jurisdiccion,
                empresa_id.as_deref(),
                dto.negocio_id.as_deref(),
                dto.activa.unwrap_or(true),
            )
            .await?,
    ))
}

pub async fn series_actualizar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(serie): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<crate::facturacion::SerieComprobante>, ApiError> {
    let usuario = contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: ActualizarSerieRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    if !usuario.es_admin_global() {
        let propietario = state.facturacion.empresa_de_serie(&serie).await?;
        if propietario.as_deref() != Some(usuario.tenant()?) {
            return Err(ApiError::acceso_denegado(
                "Solo puedes administrar las series de tu empresa.",
            ));
        }
    }
    Ok(Json(
        state
            .facturacion
            .actualizar_serie(&serie, dto.activa, dto.prefijo.as_deref())
            .await?,
    ))
}

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
            .listar(
                desde,
                hasta,
                query.get("estado").map(String::as_str),
                query.get("negocioId").map(String::as_str),
            )
            .await?,
    ))
}

pub async fn comprobantes_series(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<SerieComprobante>>, ApiError> {
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
            .facturacion
            .series_de(
                query.get("negocioId").map(String::as_str),
                empresa_id.as_deref(),
            )
            .await?,
    ))
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

/// `GET /api/v2/finanzas/comprobantes/{id}/pdf` (capacidad electrónica).
pub async fn comprobante_pdf(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<axum::response::Response, ApiError> {
    let usuario = contexto(
        &state,
        &headers,
        &ROLES_FINANZAS,
        &[],
        &[CAP_FACTURACION, "facturacion_electronica"],
    )
    .await?;
    let comprobante = state
        .facturacion
        .obtener(&id)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Comprobante", &id))?;
    let empresa = comprobante
        .empresa_id
        .as_deref()
        .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Comprobante sin empresa."))?;
    let pdf = state
        .facturacion
        .pdf_de(&id, empresa, comprobante.negocio_id.as_deref())
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Comprobante", &id))?;
    let _ = usuario;
    axum::response::Response::builder()
        .header(axum::http::header::CONTENT_TYPE, "application/pdf")
        .header(
            axum::http::header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}-{}.pdf\"",
                comprobante.serie, comprobante.numero
            ),
        )
        .body(axum::body::Body::from(pdf))
        .map_err(|_| ApiError::interno())
}

// ── configuración fiscal por tenant ─────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ConfiguracionFiscalRequest {
    #[serde(default)]
    pub jurisdiccion: Option<String>,
    #[serde(default)]
    pub empresa_id: Option<String>,
    #[serde(default)]
    pub negocio_id: Option<String>,
    #[serde(default)]
    pub facturacion_electronica: Option<bool>,
    #[serde(default)]
    pub formato_pdf: Option<bool>,
    #[serde(default)]
    pub proveedor_electronico: Option<String>,
    #[serde(default)]
    pub ambiente_electronico: Option<String>,
    #[serde(default)]
    pub impuesto_activo: Option<bool>,
    #[serde(default)]
    pub precios_con_impuesto: Option<bool>,
}

fn empresa_fiscal(
    usuario: &crate::auth::UsuarioContexto,
    empresa: Option<&String>,
) -> Result<String, ApiError> {
    if usuario.es_admin_global() {
        return empresa
            .map(String::as_str)
            .map(str::trim)
            .filter(|valor| !valor.is_empty())
            .map(str::to_string)
            .or_else(|| usuario.tenant_id.clone())
            .ok_or_else(|| {
                ApiError::validacion(json!([
                    {"campo": "empresa_id", "restricciones": ["isString"]}
                ]))
            });
    }
    Ok(usuario.tenant()?.to_string())
}

/// Estado de certificación que se publica al frontend/operación. No se infiere
/// que un proveedor esté certificado solo por estar configurado.
fn certificacion_fiscal(
    state: &AppState,
    config: &crate::tributacion::ConfiguracionFacturacion,
) -> (&'static str, bool) {
    if !config.facturacion_electronica {
        return ("no_aplica", false);
    }
    if config.proveedor_electronico == "simulado" {
        return if state.config.facturacion_permitir_sin_certificado {
            ("piloto_sin_comprobante", false)
        } else {
            ("no_certificada", false)
        };
    }
    if state.config.facturacion_proveedor_certificado {
        ("certificada", true)
    } else {
        ("no_certificada", false)
    }
}

/// `GET /api/v2/finanzas/configuracion-fiscal?empresa_id=&negocio_id=`.
pub async fn configuracion_fiscal_obtener(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let usuario = contexto(
        &state,
        &headers,
        &["super_admin", "admin", "empresario"],
        &[],
        &[],
    )
    .await?;
    let empresa = empresa_fiscal(&usuario, query.get("empresa_id"))?;
    let negocio = query
        .get("negocio_id")
        .cloned()
        .or_else(|| usuario.negocio_id.clone());
    let config = state
        .tributacion
        .configuracion_facturacion(&empresa, negocio.as_deref())
        .await?;
    let (certificacion_estado, emite_comprobante_fiscal) = certificacion_fiscal(&state, &config);
    Ok(Json(json!({
        "empresa_id": empresa,
        "negocio_id": negocio,
        "jurisdiccion": config.jurisdiccion,
        "facturacion_electronica": config.facturacion_electronica,
        "formato_pdf": config.formato_pdf,
        "proveedor_electronico": config.proveedor_electronico,
        "ambiente_electronico": config.ambiente_electronico,
        "certificacion_estado": certificacion_estado,
        "emite_comprobante_fiscal": emite_comprobante_fiscal,
        "tasa_iva": config.jurisdiccion.tasa_iva,
        "moneda": config.jurisdiccion.moneda,
        "moneda_iso": config.jurisdiccion.moneda_iso,
        "locale": config.jurisdiccion.locale,
        "zona_horaria": config.jurisdiccion.zona_horaria,
        "tasas": config.tasas,
        "tasa_default_codigo": config.jurisdiccion.tasa_default_codigo,
        "retencion_activa": config.retencion_activa,
        "tasa_retencion": config.tasa_retencion,
        "retencion_codigo": config.retencion_codigo,
        "impuesto_activo": config.impuesto_activo,
        "precios_con_impuesto": config.precios_con_impuesto,
    })))
}

/// `PUT /api/v2/finanzas/configuracion-fiscal`.
pub async fn configuracion_fiscal_guardar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(dto): Json<ConfiguracionFiscalRequest>,
) -> Result<Json<Value>, ApiError> {
    let usuario = contexto(
        &state,
        &headers,
        &["super_admin", "admin", "empresario"],
        &[],
        &[],
    )
    .await?;
    let empresa = empresa_fiscal(&usuario, dto.empresa_id.as_ref())?;
    let negocio = dto
        .negocio_id
        .clone()
        .or_else(|| usuario.negocio_id.clone());
    let jurisdiccion = dto.jurisdiccion.unwrap_or_default();
    let facturacion_electronica = dto.facturacion_electronica.unwrap_or(false);
    if facturacion_electronica || dto.formato_pdf.unwrap_or(false) {
        state
            .exigir_capacidades(&usuario, &["facturacion_electronica"])
            .await?;
    }
    let config = state
        .tributacion
        .guardar_configuracion_facturacion(
            &empresa,
            negocio.as_deref(),
            &jurisdiccion,
            facturacion_electronica,
            dto.formato_pdf.unwrap_or(false),
            dto.proveedor_electronico.as_deref().unwrap_or("simulado"),
            dto.ambiente_electronico.as_deref().unwrap_or("pruebas"),
            dto.impuesto_activo.unwrap_or(false),
            dto.precios_con_impuesto.unwrap_or(true),
        )
        .await?;
    let (certificacion_estado, emite_comprobante_fiscal) = certificacion_fiscal(&state, &config);
    Ok(Json(json!({
        "empresa_id": empresa,
        "negocio_id": negocio,
        "jurisdiccion": config.jurisdiccion,
        "facturacion_electronica": config.facturacion_electronica,
        "formato_pdf": config.formato_pdf,
        "proveedor_electronico": config.proveedor_electronico,
        "ambiente_electronico": config.ambiente_electronico,
        "certificacion_estado": certificacion_estado,
        "emite_comprobante_fiscal": emite_comprobante_fiscal,
        "tasa_iva": config.jurisdiccion.tasa_iva,
        "moneda": config.jurisdiccion.moneda,
        "moneda_iso": config.jurisdiccion.moneda_iso,
        "locale": config.jurisdiccion.locale,
        "zona_horaria": config.jurisdiccion.zona_horaria,
        "tasas": config.tasas,
        "tasa_default_codigo": config.jurisdiccion.tasa_default_codigo,
        "retencion_activa": config.retencion_activa,
        "tasa_retencion": config.tasa_retencion,
        "retencion_codigo": config.retencion_codigo,
        "impuesto_activo": config.impuesto_activo,
        "precios_con_impuesto": config.precios_con_impuesto,
    })))
}

// ── modelo financiero ───────────────────────────────────────────────────────
