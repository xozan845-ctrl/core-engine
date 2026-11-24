use super::*;

pub async fn proyecciones_crear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Proyeccion>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let dto: CrearProyeccionRequest = serde_json::from_value(cuerpo).map_err(validacion_body)?;
    if dto.nombre.trim().len() < 3 {
        return Err(ApiError::validacion(json!([
            {"campo": "nombre", "restricciones": ["minLength:3"]}
        ])));
    }
    validar_supuestos(&dto.supuestos)?;
    Ok(Json(
        state.proyecciones.crear(&dto.nombre, dto.supuestos).await?,
    ))
}

#[derive(Debug, Clone, Deserialize)]
pub struct CrearProyeccionRequest {
    pub nombre: String,
    pub supuestos: SupuestosProyeccion,
}

pub async fn proyecciones_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ProyeccionResumen>>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(state.proyecciones.listar().await?))
}

pub async fn proyeccion_detalle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Proyeccion>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    state
        .proyecciones
        .obtener(&id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::no_encontrado("Proyeccion", &id))
}

pub async fn proyecciones_sensibilidad(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EstudiosSensibilidad>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(Proyecciones::estudios(
        &SupuestosProyeccion::supuestos_informe(),
    )?))
}

pub async fn proyecciones_plan_bienal(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<PlanBienal>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(Proyecciones::plan_bienal(
        &SupuestosProyeccion::supuestos_informe(),
    )?))
}

pub async fn punto_equilibrio(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let horizonte = query_i64(&query, "horizonte_meses", 12).clamp(1, 60);
    let supuestos = SupuestosProyeccion {
        horizonte_meses: horizonte,
        vendedores_iniciales: 1,
        entrada_vendedores_mes: 0,
        churn_tasa: 0.0,
        pedidos_por_vendedor: (0..horizonte)
            .map(|indice| (25.0 + indice as f64 * 1.6).min(42.0))
            .collect(),
        ticket_promedio_cents: query_i64(&query, "ticket_promedio_cents", 45_000),
        comision_tasa: query_f64(&query, "comision_tasa", 0.12),
        costos_fijos_cents: query_i64(&query, "costos_fijos_cents", 300_000),
        costos_fijos_desde_mes7_cents: None,
        inversion_inicial_cents: 0,
        tasa_descuento_mensual: None,
    };
    let (filas, _) = Proyecciones::calcular(&supuestos)?;
    let indicadores = Proyecciones::indicadores(&filas, &supuestos);
    Ok(Json(json!({
        "punto_equilibrio_caja_cents": indicadores.punto_equilibrio_caja_cents,
        "pedidos_equilibrio_caja": indicadores.pedidos_equilibrio_caja,
        "escenarios_equilibrio": indicadores.escenarios_equilibrio,
        "margen_seguridad_por_mes": indicadores.margen_seguridad_por_mes,
    })))
}

pub async fn kpis(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<KpisFinancieros>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    Ok(Json(
        state
            .kpis
            .kpis(query.get("mes").map(String::as_str))
            .await?,
    ))
}

pub async fn tablero(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Tablero>, ApiError> {
    contexto(&state, &headers, &ROLES_FINANZAS, &[], &[CAP_FACTURACION]).await?;
    let costo = query
        .get("costo_entrega_por_pedido_cents")
        .and_then(|valor| valor.parse::<i64>().ok());
    Ok(Json(
        state
            .alertas
            .tablero(OpcionesTablero {
                mes: query.get("mes").cloned(),
                costo_entrega_por_pedido_cents: costo,
            })
            .await?,
    ))
}

// ── vendedor fiscal ─────────────────────────────────────────────────────────

pub async fn vendedor_fiscal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let usuario = contexto(&state, &headers, &[ROLES[3]], &[], &[]).await?;
    let sujeto = state.tributacion.sujeto_fiscal_de(&usuario.user_id).await?;
    let resumen = state
        .kpis
        .resumen_del_vendedor(
            state.contabilidad.db(),
            &usuario.user_id,
            query.get("mes").map(String::as_str),
        )
        .await?;
    Ok(Json(json!({
        "sujeto_fiscal": sujeto,
        "resumen_comisiones": resumen,
        "nota": "Comisiones documentadas para la declaracion del vendedor ante la DGI (Ley 822 / regimen de Cuota Fija).",
    })))
}
