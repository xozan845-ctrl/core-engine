use super::*;

pub async fn tipos_cambio_listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    auth(&state, &headers).await?;
    let filas = state
        .comercio
        .listar_tipos_cambio(
            params.get("base").map(String::as_str),
            params.get("quote").map(String::as_str),
        )
        .await?;
    Ok(Json(Value::Array(filas)))
}

pub async fn tipos_cambio_registrar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Json<Value>, ApiError> {
    let usuario = auth(&state, &headers).await?;
    usuario.exigir_roles(&["super_admin", "admin"], &[])?;
    let base = cuerpo
        .get("base")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let quote = cuerpo
        .get("quote")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tasa = cuerpo
        .get("tasa")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(Json(
        state
            .comercio
            .registrar_tipo_cambio(base, quote, tasa, &usuario)
            .await?,
    ))
}
