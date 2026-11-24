use super::*;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearCuentaRequest {
    pub codigo: String,
    pub nombre: String,
    pub tipo: String,
    pub naturaleza: String,
    #[serde(default)]
    pub nivel: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EstadoCuentaRequest {
    pub estado: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct DetalleRequest {
    pub cuenta_codigo: String,
    #[serde(default)]
    pub debe_cents: Option<i64>,
    #[serde(default)]
    pub haber_cents: Option<i64>,
    #[serde(default)]
    pub concepto: Option<String>,
    #[serde(default)]
    pub orden: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CrearAsientoRequest {
    pub concepto: String,
    #[serde(default)]
    pub fecha: Option<String>,
    #[serde(default)]
    pub tipo: Option<String>,
    #[serde(default)]
    pub referencia_tipo: Option<String>,
    #[serde(default)]
    pub referencia_id: Option<String>,
    #[serde(default)]
    pub detalles: Vec<DetalleRequest>,
}

pub async fn cuentas_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::contabilidad::Cuenta>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(state.contabilidad.plan_de_cuentas().await?))
}

pub async fn cuentas_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<crate::contabilidad::Cuenta>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: CrearCuentaRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    if dto.nombre.trim().len() < 3 {
        return Err(ApiError::validacion(json!([
            {"campo": "nombre", "restricciones": ["minLength:3"]}
        ])));
    }
    validar_in(
        &dto.tipo,
        "tipo",
        &["ACTIVO", "PASIVO", "CAPITAL", "INGRESO", "COSTO", "GASTO"],
    )?;
    validar_in(&dto.naturaleza, "naturaleza", &["DEUDORA", "ACREEDORA"])?;
    let cuenta = state
        .contabilidad
        .crear_cuenta(
            &dto.codigo,
            &dto.nombre,
            &dto.tipo,
            &dto.naturaleza,
            dto.nivel.unwrap_or(3),
        )
        .await?;
    Ok(Json(cuenta))
}

pub async fn cuenta_estado(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(codigo): Path<String>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<crate::contabilidad::Cuenta>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: EstadoCuentaRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    validar_in(&dto.estado, "estado", &["activa", "inactiva"])?;
    state
        .contabilidad
        .estado_cuenta(&codigo, &dto.estado)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Cuenta", &codigo))
}

pub async fn asientos_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<crate::contabilidad::Asiento>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let desde = fecha_query(query.get("desde"))?;
    let hasta = fecha_query(query.get("hasta"))?;
    let limite = query_i64(&query, "limite", 200);
    Ok(Json(
        state
            .contabilidad
            .libro_diario(desde, hasta, limite)
            .await?,
    ))
}

pub async fn asiento_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<crate::contabilidad::Asiento>, ApiError> {
    let usuario = contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: CrearAsientoRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    if dto.concepto.trim().len() < 3 {
        return Err(ApiError::validacion(json!([
            {"campo": "concepto", "restricciones": ["minLength:3"]}
        ])));
    }
    if dto.detalles.len() < 2 {
        return Err(ApiError::validacion(json!([
            {"campo": "detalles", "restricciones": ["arrayMinSize"]}
        ])));
    }
    let fecha = fecha_query(dto.fecha.as_ref())?;
    let detalles: Vec<DetalleAsiento> = dto
        .detalles
        .into_iter()
        .map(|detalle| DetalleAsiento {
            cuenta_codigo: detalle.cuenta_codigo,
            debe_cents: detalle.debe_cents.unwrap_or(0),
            haber_cents: detalle.haber_cents.unwrap_or(0),
            concepto: detalle.concepto,
            orden: detalle.orden,
        })
        .collect();
    let asiento = state
        .contabilidad
        .registrar(ParametrosAsiento {
            concepto: dto.concepto,
            tipo: dto.tipo.unwrap_or_else(|| "MANUAL".to_string()),
            referencia_tipo: dto.referencia_tipo,
            referencia_id: dto.referencia_id,
            fecha,
            moneda: None,
            creado_por: Some(usuario.user_id),
            detalles,
        })
        .await?;
    Ok(Json(asiento))
}

pub async fn asiento_anular(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::contabilidad::Asiento>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    state
        .contabilidad
        .anular(&id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Asiento", &id))
}

pub async fn libro_mayor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cuenta): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<crate::contabilidad::LibroMayor>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let hasta = fecha_query(query.get("hasta"))?;
    Ok(Json(state.contabilidad.libro_mayor(&cuenta, hasta).await?))
}

pub async fn libro_ventas(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<crate::contabilidad::LibroIva>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let desde = fecha_query(query.get("desde"))?;
    let hasta = fecha_query(query.get("hasta"))?;
    Ok(Json(state.contabilidad.libro_ventas(desde, hasta).await?))
}

pub async fn libro_ventas_negocios(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let usuario = contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let desde = fecha_query(query.get("desde"))?;
    let hasta = fecha_query(query.get("hasta"))?;
    let negocio = usuario
        .negocio_id
        .clone()
        .or_else(|| query.get("negocioId").cloned().filter(|v| !v.is_empty()));
    Ok(Json(
        state
            .contabilidad
            .libro_ventas_negocios(desde, hasta, negocio.as_deref())
            .await?,
    ))
}

pub async fn libro_compras(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Vec<crate::contabilidad::LibroIva>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let desde = fecha_query(query.get("desde"))?;
    let hasta = fecha_query(query.get("hasta"))?;
    Ok(Json(state.contabilidad.libro_compras(desde, hasta).await?))
}

// ── tributación ─────────────────────────────────────────────────────────────
