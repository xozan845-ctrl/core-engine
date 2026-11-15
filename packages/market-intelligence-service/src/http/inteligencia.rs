//! Rutas HTTP de inteligencia: métricas, mapas de calor, demanda y predicción.

use crate::auth::UsuarioContexto;
use crate::errors::ApiError;
use crate::http::Cuerpo;
use crate::inteligencia::{parse_int_js, Filtros, VentaGeolocalizada};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const CAP_INTELIGENCIA: &[&str] = &["inteligencia_mercado"];

type Consulta = HashMap<String, String>;

async fn autorizar(state: &AppState, headers: &HeaderMap) -> Result<UsuarioContexto, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(headers)?;
    state.exigir_capacidades(&usuario, CAP_INTELIGENCIA).await?;
    Ok(usuario)
}

fn personal(usuario: &UsuarioContexto) -> Result<String, ApiError> {
    usuario.personal_id.clone().ok_or_else(|| {
        ApiError::no_autorizado(
            "La ruta /me requiere una ficha de personal vinculada (x-user-personal).",
        )
    })
}

// ── Validación de query (FiltrosInteligenciaDto) ──────────────────────────

fn filtros_validados(q: &Consulta) -> Result<Filtros, ApiError> {
    let mut errores = Vec::new();
    for campo in ["desde", "hasta"] {
        if let Some(valor) = q.get(campo) {
            if !es_fecha_iso(valor) {
                errores.push(json!({"campo": campo, "restricciones": ["isDateString"]}));
            }
        }
    }
    if let Some(valor) = q.get("vendedor_id") {
        if !es_uuid_v4(valor) {
            errores.push(json!({"campo": "vendedor_id", "restricciones": ["isUuid"]}));
        }
    }
    if !errores.is_empty() {
        return Err(ApiError::validacion(Value::Array(errores)));
    }
    Ok(Filtros {
        desde: q.get("desde").cloned(),
        hasta: q.get("hasta").cloned(),
        sku: q.get("sku").cloned(),
        vendedor_id: q.get("vendedor_id").cloned(),
        ..Default::default()
    })
}

fn filtros_crudos(q: &Consulta) -> Filtros {
    Filtros {
        sku: q.get("sku").cloned(),
        vendedor_id: q.get("vendedor_id").cloned(),
        cursor: q.get("cursor").cloned(),
        empresa_id: q.get("empresa_id").cloned(),
        fecha_inicio: q.get("fecha_inicio").cloned(),
        fecha_fin: q.get("fecha_fin").cloned(),
        ..Default::default()
    }
}

fn limite(q: &Consulta, defecto: i32) -> Result<i32, ApiError> {
    match q.get("limite") {
        Some(valor) => parse_int_js(valor).ok_or_else(ApiError::interno),
        None => Ok(defecto),
    }
}

fn numero(q: &Consulta, campo: &str) -> Result<Option<f64>, ApiError> {
    match q.get(campo) {
        Some(valor) => valor
            .parse::<f64>()
            .map(Some)
            .map_err(|_| ApiError::interno()),
        None => Ok(None),
    }
}

// ── Handlers ──────────────────────────────────────────────────────────────

pub async fn mapa_calor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let filtros = filtros_validados(&q)?;
    Ok(Json(state.inteligencia.obtener_mapa_calor(&filtros).await?))
}

pub async fn mi_mapa_calor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let vendedor = personal(&usuario)?;
    let mut filtros = filtros_validados(&q)?;
    filtros.vendedor_id = Some(vendedor);
    Ok(Json(state.inteligencia.obtener_mapa_calor(&filtros).await?))
}

pub async fn rendimiento(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let mut filtros = filtros_crudos(&q);
    filtros.limite = Some(limite(&q, 50)?);
    Ok(Json(
        state.inteligencia.obtener_rendimiento(&filtros).await?,
    ))
}

pub async fn mi_rendimiento(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let vendedor = personal(&usuario)?;
    let mut filtros = filtros_crudos(&q);
    filtros.vendedor_id = Some(vendedor);
    filtros.limite = Some(limite(&q, 50)?);
    Ok(Json(
        state.inteligencia.obtener_rendimiento(&filtros).await?,
    ))
}

pub async fn cobertura(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let mut filtros = Filtros {
        limite: Some(limite(&q, 100)?),
        ..Default::default()
    };
    filtros.lat_min = numero(&q, "lat_min")?;
    filtros.lat_max = numero(&q, "lat_max")?;
    filtros.lng_min = numero(&q, "lng_min")?;
    filtros.lng_max = numero(&q, "lng_max")?;
    Ok(Json(state.inteligencia.obtener_cobertura(&filtros).await?))
}

pub async fn demanda(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let mut filtros = filtros_crudos(&q);
    filtros.limite = Some(limite(&q, 50)?);
    Ok(Json(state.inteligencia.obtener_demanda(&filtros).await?))
}

pub async fn tendencias(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let filtros = filtros_crudos(&q);
    Ok(Json(state.inteligencia.obtener_tendencias(&filtros).await?))
}

pub async fn resumen(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let filtros = filtros_validados(&q)?;
    Ok(Json(state.inteligencia.obtener_resumen(&filtros).await?))
}

pub async fn registrar_venta(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let usuario = autorizar(&state, &headers).await?;
    let mut venta = validar_venta(&cuerpo)?;

    // Un vendedor solo registra sus propias ventas: el claim manda sobre el cuerpo.
    if usuario.rol == "vendedor" {
        let personal = usuario.personal_id.clone().ok_or_else(|| {
            ApiError::acceso_denegado(
                "El usuario vendedor no tiene ficha de personal vinculada (x-user-personal).",
            )
        })?;
        venta.vendedor_id = personal;
    }

    let evento_id = cuerpo
        .get("evento_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    state
        .inteligencia
        .registrar_venta(&venta, evento_id.as_deref())
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "codigo": "OK",
            "mensaje": "Venta registrada en inteligencia",
        })),
    ))
}

pub async fn forecast(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let sku = q.get("sku").map(String::as_str);
    let dias = match q.get("dias").and_then(|valor| parse_int_js(valor)) {
        Some(valor) if valor != 0 => valor.clamp(1, 90),
        _ => 7,
    };
    Ok(Json(state.inteligencia.obtener_forecast(sku, dias).await?))
}

pub async fn anomalias(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let limite = match q.get("limite").and_then(|valor| parse_int_js(valor)) {
        Some(valor) if valor != 0 => valor.min(200),
        _ => 50,
    };
    let filtros = Filtros {
        sku: q.get("sku").cloned(),
        vendedor_id: q.get("vendedor_id").cloned(),
        limite: Some(limite),
        ..Default::default()
    };
    Ok(Json(state.inteligencia.obtener_anomalias(&filtros).await?))
}

pub async fn score_vendedor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let score = state.inteligencia.obtener_score(&id).await?;
    Ok(Json(score.unwrap_or_else(|| {
        json!({
            "vendedor_id": id,
            "mensaje": "Sin datos computados aún. Las features se calculan cada 5 minutos.",
        })
    })))
}

pub async fn mi_score(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let Some(user_id) = headers
        .get("x-user-id")
        .and_then(|valor| valor.to_str().ok())
        .filter(|valor| !valor.is_empty())
    else {
        return Ok(Json(json!({
            "mensaje": "x-user-id header requerido (inyectado por el gateway)",
        })));
    };
    let score = state.inteligencia.obtener_score(user_id).await?;
    Ok(Json(score.unwrap_or_else(|| {
        json!({
            "vendedor_id": user_id,
            "mensaje": "Sin datos computados aún.",
        })
    })))
}

pub async fn calidad_datos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<Consulta>,
) -> Result<Json<Value>, ApiError> {
    autorizar(&state, &headers).await?;
    let horas = match q.get("horas").and_then(|valor| parse_int_js(valor)) {
        Some(valor) if valor != 0 => valor.min(720),
        _ => 24,
    };
    Ok(Json(state.inteligencia.obtener_calidad(horas).await?))
}

// ── Validación del body de ingesta (RegistrarVentaRequestDto) ─────────────

fn validar_venta(valor: &Value) -> Result<VentaGeolocalizada, ApiError> {
    let objeto = valor.as_object().cloned().unwrap_or_default();
    let mut errores = Vec::new();

    let order_id = validar_uuid_requerido(&objeto, "order_id", &mut errores);
    let vendedor_id = validar_uuid_requerido(&objeto, "vendedor_id", &mut errores);
    let skus = validar_skus(&objeto, &mut errores);
    let monto_cents = validar_monto(&objeto, &mut errores);
    let lat = validar_numero(
        &objeto,
        "lat",
        Some(-90.0),
        Some(90.0),
        "isLatitude",
        &mut errores,
    );
    let lng = validar_numero(
        &objeto,
        "lng",
        Some(-180.0),
        Some(180.0),
        "isLongitude",
        &mut errores,
    );
    let precision = validar_numero(
        &objeto,
        "precision",
        Some(0.0),
        None,
        "isNumber",
        &mut errores,
    );
    let velocidad = validar_numero(
        &objeto,
        "velocidad",
        Some(0.0),
        None,
        "isNumber",
        &mut errores,
    );
    let rumbo = validar_numero(
        &objeto,
        "rumbo",
        Some(0.0),
        Some(360.0),
        "isNumber",
        &mut errores,
    );
    let distancia = validar_numero(
        &objeto,
        "distancia_cliente_metros",
        Some(0.0),
        None,
        "isNumber",
        &mut errores,
    );
    let tipo_actividad = validar_enum(
        &objeto,
        "tipo_actividad",
        &["impulsacion", "venta", "entrega", "reparto"],
        &mut errores,
    );
    let resultado_visita = validar_enum(
        &objeto,
        "resultado_visita",
        &["visitado", "cerca_no_visitado", "no_visitado"],
        &mut errores,
    );
    let rango_edad = validar_enum(
        &objeto,
        "rango_edad",
        &["18-24", "25-34", "35-44", "45-54", "55+"],
        &mut errores,
    );
    let genero = validar_enum(&objeto, "genero", &["M", "F", "NS"], &mut errores);
    if let Some(evento) = objeto.get("evento_id") {
        if !evento.is_string() || !es_uuid(evento.as_str().unwrap_or_default()) {
            errores.push(json!({"campo": "evento_id", "restricciones": ["isUuid"]}));
        }
    }

    if !errores.is_empty() {
        return Err(ApiError::validacion(Value::Array(errores)));
    }

    Ok(VentaGeolocalizada {
        order_id: order_id.expect("order_id validado"),
        vendedor_id: vendedor_id.expect("vendedor_id validado"),
        skus: skus.expect("skus validado"),
        monto_cents: monto_cents.expect("monto validado"),
        lat,
        lng,
        precision,
        velocidad,
        rumbo,
        tipo_actividad,
        resultado_visita,
        distancia_cliente_metros: distancia,
        rango_edad,
        genero,
    })
}

fn validar_uuid_requerido(
    objeto: &serde_json::Map<String, Value>,
    campo: &str,
    errores: &mut Vec<Value>,
) -> Option<String> {
    let valor = objeto.get(campo).and_then(Value::as_str);
    match valor {
        Some(texto) if es_uuid(texto) => Some(texto.to_string()),
        _ => {
            errores.push(json!({"campo": campo, "restricciones": ["isUuid"]}));
            None
        }
    }
}

fn validar_skus(
    objeto: &serde_json::Map<String, Value>,
    errores: &mut Vec<Value>,
) -> Option<Vec<String>> {
    let Some(valor) = objeto.get("skus") else {
        errores.push(json!({"campo": "skus", "restricciones": ["isArray", "arrayMinSize"]}));
        return None;
    };
    let Some(items) = valor.as_array() else {
        errores.push(json!({"campo": "skus", "restricciones": ["isArray"]}));
        return None;
    };
    let mut restricciones = Vec::new();
    if items.is_empty() {
        restricciones.push("arrayMinSize");
    }
    if items.iter().any(|item| !item.is_string()) {
        restricciones.push("isString");
    }
    if !restricciones.is_empty() {
        errores.push(json!({"campo": "skus", "restricciones": restricciones}));
        return None;
    }
    Some(
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
    )
}

fn validar_monto(objeto: &serde_json::Map<String, Value>, errores: &mut Vec<Value>) -> Option<i64> {
    let Some(valor) = objeto.get("monto_cents") else {
        errores.push(json!({"campo": "monto_cents", "restricciones": ["isInt", "min"]}));
        return None;
    };
    let Some(numero) = valor.as_i64() else {
        errores.push(json!({"campo": "monto_cents", "restricciones": ["isInt"]}));
        return None;
    };
    if numero < 0 {
        errores.push(json!({"campo": "monto_cents", "restricciones": ["min"]}));
        return None;
    }
    Some(numero)
}

fn validar_numero(
    objeto: &serde_json::Map<String, Value>,
    campo: &str,
    minimo: Option<f64>,
    maximo: Option<f64>,
    restriccion: &str,
    errores: &mut Vec<Value>,
) -> Option<f64> {
    let valor = objeto.get(campo)?;
    let Some(numero) = valor.as_f64() else {
        errores.push(json!({"campo": campo, "restricciones": [restriccion]}));
        return None;
    };
    let mut fallos = Vec::new();
    if minimo.is_some_and(|limite| numero < limite) {
        fallos.push("min");
    }
    if maximo.is_some_and(|limite| numero > limite) {
        fallos.push("max");
    }
    if !fallos.is_empty() {
        let mut restricciones = vec![restriccion];
        restricciones.extend(fallos);
        errores.push(json!({"campo": campo, "restricciones": restricciones}));
        return None;
    }
    Some(numero)
}

fn validar_enum(
    objeto: &serde_json::Map<String, Value>,
    campo: &str,
    permitidos: &[&str],
    errores: &mut Vec<Value>,
) -> Option<String> {
    let valor = objeto.get(campo)?;
    match valor.as_str() {
        Some(texto) if permitidos.contains(&texto) => Some(texto.to_string()),
        _ => {
            errores.push(json!({"campo": campo, "restricciones": ["isIn"]}));
            None
        }
    }
}

// ── Helpers de formato ────────────────────────────────────────────────────

fn es_uuid(valor: &str) -> bool {
    uuid::Uuid::parse_str(valor).is_ok()
}

fn es_uuid_v4(valor: &str) -> bool {
    uuid::Uuid::parse_str(valor)
        .map(|uuid| uuid.get_version_num() == 4)
        .unwrap_or(false)
}

fn es_fecha_iso(valor: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(valor).is_ok()
        || chrono::NaiveDate::parse_from_str(valor, "%Y-%m-%d").is_ok()
        || chrono::NaiveDateTime::parse_from_str(valor, "%Y-%m-%dT%H:%M:%S").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debe_validar_los_filtros_como_el_dto() {
        let mut q = HashMap::new();
        q.insert("desde".to_string(), "2026-09-01".to_string());
        q.insert(
            "vendedor_id".to_string(),
            "00000000-0000-4000-8000-000000000001".to_string(),
        );
        let filtros = filtros_validados(&q).expect("validos");
        assert_eq!(filtros.sku, None);
        assert_eq!(
            filtros.vendedor_id.as_deref(),
            Some("00000000-0000-4000-8000-000000000001")
        );

        q.insert("vendedor_id".to_string(), "no-uuid".to_string());
        assert_eq!(
            filtros_validados(&q).expect_err("invalido").codigo,
            "VALIDACION"
        );
    }

    #[test]
    fn debe_validar_el_body_de_ingesta() {
        let valida = validar_venta(&json!({
            "order_id": "00000000-0000-4000-8000-000000000001",
            "vendedor_id": "00000000-0000-4000-8000-000000000002",
            "skus": ["SKU-1"],
            "monto_cents": 1000,
            "lat": 12.1,
            "lng": -86.2,
            "genero": "M",
        }))
        .expect("valida");
        assert_eq!(valida.skus, vec!["SKU-1"]);
        assert_eq!(valida.genero.as_deref(), Some("M"));

        let invalida = validar_venta(&json!({
            "order_id": "no-uuid",
            "skus": [],
            "monto_cents": -1,
            "lat": 91,
            "genero": "X",
        }))
        .expect_err("invalida");
        let detalles = invalida.cuerpo()["detalles"].clone();
        assert!(detalles.as_array().expect("detalles").len() >= 5);
    }

    #[test]
    fn debe_replicar_los_defaults_de_dias_limite_y_horas() {
        assert_eq!(
            match Some(500).filter(|valor| *valor != 0) {
                Some(valor) => valor.clamp(1, 90),
                None => 7,
            },
            90
        );
        assert_eq!(
            match Some(9999).filter(|valor| *valor != 0) {
                Some(valor) => valor.min(200),
                None => 50,
            },
            200
        );
        assert_eq!(
            match Some(9999).filter(|valor| *valor != 0) {
                Some(valor) => valor.min(720),
                None => 24,
            },
            720
        );
    }

    #[test]
    fn debe_reconocer_fechas_y_uuid_v4() {
        assert!(es_fecha_iso("2026-09-01"));
        assert!(es_fecha_iso("2026-09-01T10:00:00.000Z"));
        assert!(!es_fecha_iso("2026-13-01"));
        assert!(es_uuid_v4("00000000-0000-4000-8000-000000000001"));
        assert!(!es_uuid_v4("00000000-0000-1000-8000-000000000001"));
    }
}
