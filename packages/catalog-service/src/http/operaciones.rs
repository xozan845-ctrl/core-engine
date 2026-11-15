//! Handlers de operaciones de catálogo (configuración, importación, escaneo,
//! combos, promociones y movimientos de inventario).

use super::validacion::{self, campo, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::{UsuarioContexto, ROLES_CATALOGO};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use std::collections::HashMap;

const CAP_ITEMS: &[&str] = &["catalogo_items"];
const CAP_STOCK: &[&str] = &["inventario_stock", "catalogo_items"];
const CAP_BARRAS: &[&str] = &["codigo_barras", "catalogo_items"];

const CONFIGURACION: &[ReglaCampo] = &[
    campo(
        "metodo_costeo",
        false,
        &[Regla::En(&["promedio", "fifo", "ueps"])],
    ),
    campo("permitir_stock_negativo", false, &[Regla::Booleano]),
    campo("alerta_stock_bajo", false, &[Regla::Booleano]),
    campo("actualizar_costo_en_compra", false, &[Regla::Booleano]),
    campo("unidad_defecto", false, &[Regla::Texto]),
];

const COMBO: &[ReglaCampo] = &[
    campo("sku", true, &[Regla::Texto, Regla::Coincide("sku")]),
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 200)]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("codigo_barras", false, &[Regla::Texto]),
    campo("precio", true, &[Regla::Texto, Regla::Coincide("monto")]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
    campo("componentes", true, &[Regla::Arreglo]),
];

const COMBO_ACTUALIZAR: &[ReglaCampo] = &[
    campo("nombre", false, &[Regla::Texto, Regla::Longitud(2, 200)]),
    campo("descripcion", false, &[Regla::Texto]),
    campo("codigo_barras", false, &[Regla::Texto]),
    campo("precio", false, &[Regla::Texto, Regla::Coincide("monto")]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
    campo("componentes", false, &[Regla::Arreglo]),
];

const PROMOCION: &[ReglaCampo] = &[
    campo("nombre", true, &[Regla::Texto, Regla::Longitud(2, 200)]),
    campo(
        "tipo",
        true,
        &[Regla::En(&[
            "2x1",
            "descuento_cantidad",
            "descuento_porcentaje",
            "regalo",
        ])],
    ),
    campo("item_id", false, &[Regla::Uuid]),
    campo("sku", false, &[Regla::Texto]),
    campo(
        "cantidad_requerida",
        false,
        &[Regla::Entero, Regla::Min(1.0)],
    ),
    campo("cantidad_gratis", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo(
        "descuento_porcentaje",
        false,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(100.0)],
    ),
    campo("fecha_desde", true, &[Regla::Fecha]),
    campo("fecha_hasta", true, &[Regla::Fecha]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
];

const PROMOCION_ACTUALIZAR: &[ReglaCampo] = &[
    campo("nombre", false, &[Regla::Texto, Regla::Longitud(2, 200)]),
    campo(
        "tipo",
        false,
        &[Regla::En(&[
            "2x1",
            "descuento_cantidad",
            "descuento_porcentaje",
            "regalo",
        ])],
    ),
    campo("item_id", false, &[Regla::Uuid]),
    campo("sku", false, &[Regla::Texto]),
    campo(
        "cantidad_requerida",
        false,
        &[Regla::Entero, Regla::Min(1.0)],
    ),
    campo("cantidad_gratis", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo(
        "descuento_porcentaje",
        false,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(100.0)],
    ),
    campo("fecha_desde", false, &[Regla::Fecha]),
    campo("fecha_hasta", false, &[Regla::Fecha]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
];

const MOVIMIENTO: &[ReglaCampo] = &[
    campo(
        "tipo",
        true,
        &[Regla::En(&["entrada", "salida", "ajuste", "traslado"])],
    ),
    campo("motivo", false, &[Regla::Texto]),
    campo("referencia", false, &[Regla::Texto]),
    campo("ubicacion_origen", false, &[Regla::Texto]),
    campo("ubicacion_destino", false, &[Regla::Texto]),
    campo("lineas", true, &[Regla::Arreglo]),
];

fn validar(esquema: &[ReglaCampo], cuerpo: &Value) -> Result<(), ApiError> {
    validacion::validar(esquema, cuerpo)
}

fn ok(valor: Value) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

fn created(valor: Value) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

fn sin_contenido() -> Result<Response, ApiError> {
    Ok(StatusCode::NO_CONTENT.into_response())
}

fn empresa(usuario: &UsuarioContexto, solicitada: Option<&str>) -> Result<String, ApiError> {
    if usuario.rol == "empresario" {
        let tenant = usuario
            .tenant_id
            .as_deref()
            .ok_or_else(|| ApiError::no_encontrado("Empresa", "activa"))?;
        if let Some(solicitada) = solicitada {
            if !solicitada.is_empty() && solicitada != tenant {
                return Err(ApiError::dominio(
                    "EMPRESA_NO_AUTORIZADA",
                    "El empresario solo puede operar el catálogo de su empresa.",
                ));
            }
        }
        return Ok(tenant.to_string());
    }
    solicitada
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .or_else(|| usuario.tenant_id.clone())
        .ok_or_else(|| {
            ApiError::dominio(
                "EMPRESA_REQUERIDA",
                "Indica empresa_id para operar el catálogo.",
            )
        })
}

fn entero_de_query(valor: &str) -> Value {
    match valor.trim().parse::<f64>() {
        Ok(numero) if numero.is_finite() => json!(numero),
        _ => Value::String(valor.to_string()),
    }
}

fn validar_query(esquema: &[ReglaCampo], query: &HashMap<String, String>) -> Result<(), ApiError> {
    let mut objeto = serde_json::Map::new();
    for (clave, valor) in query {
        if clave == "pagina" || clave == "limite" {
            objeto.insert(clave.clone(), entero_de_query(valor));
        } else {
            objeto.insert(clave.clone(), Value::String(valor.clone()));
        }
    }
    validacion::validar(esquema, &Value::Object(objeto))
}

fn valida_sku(sku: &str) -> bool {
    (2..=32).contains(&sku.len()) && sku.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn valida_monto(monto: &str) -> bool {
    let partes: Vec<&str> = monto.split('.').collect();
    (partes.len() == 1 || partes.len() == 2)
        && !partes[0].is_empty()
        && partes[0].chars().all(|c| c.is_ascii_digit())
        && partes
            .get(1)
            .map(|d| (1..=2).contains(&d.len()) && d.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(true)
}

fn entrada_invalida() -> ApiError {
    ApiError::validacion(json!([{ "campo": "filas", "restricciones": [] }]))
}

fn validar_filas_importacion(filas: &Value) -> Result<(), ApiError> {
    let Some(filas) = filas.as_array() else {
        return Err(entrada_invalida());
    };
    if filas.is_empty() || filas.len() > 200 {
        return Err(entrada_invalida());
    }
    for fila in filas {
        let valido = fila.as_object().is_some_and(|objeto| {
            let sku = objeto.get("sku").and_then(Value::as_str).unwrap_or("");
            let nombre = objeto.get("nombre").and_then(Value::as_str).unwrap_or("");
            let precio = objeto
                .get("precio_base")
                .and_then(Value::as_str)
                .unwrap_or("");
            let stock = objeto.get("stock").and_then(Value::as_i64);
            valida_sku(sku)
                && nombre.chars().count() >= 2
                && valida_monto(precio)
                && stock.is_some_and(|valor| valor >= 0)
                && objeto
                    .get("costo")
                    .and_then(Value::as_str)
                    .map(valida_monto)
                    .unwrap_or(true)
        });
        if !valido {
            return Err(entrada_invalida());
        }
    }
    Ok(())
}

fn validar_componentes(valor: &Value, campo: &'static str) -> Result<(), ApiError> {
    let Some(componentes) = valor.as_array() else {
        return Err(ApiError::validacion(
            json!([{ "campo": campo, "restricciones": ["isArray"] }]),
        ));
    };
    if componentes.is_empty() {
        return Err(ApiError::validacion(
            json!([{ "campo": campo, "restricciones": ["arrayMinSize"] }]),
        ));
    }
    for componente in componentes {
        let valido = componente.as_object().is_some_and(|objeto| {
            objeto
                .get("sku")
                .and_then(Value::as_str)
                .is_some_and(|sku| !sku.is_empty())
                && objeto
                    .get("cantidad")
                    .and_then(Value::as_i64)
                    .is_some_and(|cantidad| cantidad >= 1)
        });
        if !valido {
            return Err(ApiError::validacion(
                json!([{ "campo": campo, "restricciones": [] }]),
            ));
        }
    }
    Ok(())
}

fn validar_lineas_movimiento(valor: &Value) -> Result<(), ApiError> {
    let Some(lineas) = valor.as_array() else {
        return Err(ApiError::validacion(
            json!([{ "campo": "lineas", "restricciones": ["isArray"] }]),
        ));
    };
    if lineas.is_empty() {
        return Err(ApiError::validacion(
            json!([{ "campo": "lineas", "restricciones": ["arrayMinSize"] }]),
        ));
    }
    for linea in lineas {
        let valido = linea.as_object().is_some_and(|objeto| {
            objeto
                .get("sku")
                .and_then(Value::as_str)
                .is_some_and(|sku| !sku.is_empty())
                && objeto
                    .get("cantidad")
                    .and_then(Value::as_i64)
                    .is_some_and(|cantidad| cantidad >= 0)
        });
        if !valido {
            return Err(ApiError::validacion(
                json!([{ "campo": "lineas", "restricciones": [] }]),
            ));
        }
    }
    Ok(())
}

/// `GET /api/v2/catalog/configuracion-inventario`.
pub async fn configuracion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_STOCK).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state.operaciones()?.configuracion(&empresa).await?)
}

/// `PUT /api/v2/catalog/configuracion-inventario`.
pub async fn guardar_configuracion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_STOCK).await?;
    validar(CONFIGURACION, &cuerpo)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .operaciones()?
        .guardar_configuracion(&empresa, &cuerpo)
        .await?)
}

/// `POST /api/v2/catalog/items/importar`.
pub async fn importar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    validar_filas_importacion(cuerpo.get("filas").unwrap_or(&Value::Null))?;
    let filas = cuerpo
        .get("filas")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    created(state.operaciones()?.importar(&empresa, &filas).await?)
}

/// `GET /api/v2/catalog/escanear`.
pub async fn escanear(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_BARRAS).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .operaciones()?
        .escanear(
            &empresa,
            query.get("codigo").map(String::as_str).unwrap_or(""),
        )
        .await?)
}

const LISTAR_COMBOS: &[ReglaCampo] = &[
    campo("empresa_id", false, &[Regla::Uuid]),
    campo("q", false, &[Regla::Texto]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
    campo("pagina", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo(
        "limite",
        false,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(100.0)],
    ),
];

/// `GET /api/v2/catalog/combos`.
pub async fn listar_combos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    validar_query(LISTAR_COMBOS, &query)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .operaciones()?
        .listar_combos(&empresa, &json!(query))
        .await?)
}

/// `POST /api/v2/catalog/combos`.
pub async fn crear_combo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    validar(COMBO, &cuerpo)?;
    validar_componentes(
        cuerpo.get("componentes").unwrap_or(&Value::Null),
        "componentes",
    )?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    created(state.operaciones()?.crear_combo(&empresa, &cuerpo).await?)
}

/// `GET /api/v2/catalog/combos/:id`.
pub async fn detalle_combo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    let combo = state
        .operaciones()?
        .obtener_combo(&id, &empresa)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Combo", &id))?;
    ok(combo)
}

/// `PATCH /api/v2/catalog/combos/:id`.
pub async fn actualizar_combo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    validar(COMBO_ACTUALIZAR, &cuerpo)?;
    if let Some(componentes) = cuerpo.get("componentes") {
        validar_componentes(componentes, "componentes")?;
    }
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .operaciones()?
        .actualizar_combo(&id, &empresa, &cuerpo)
        .await?)
}

/// `DELETE /api/v2/catalog/combos/:id`.
pub async fn eliminar_combo(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    state.operaciones()?.eliminar_combo(&id, &empresa).await?;
    sin_contenido()
}

const LISTAR_PROMOCIONES: &[ReglaCampo] = &[
    campo("empresa_id", false, &[Regla::Uuid]),
    campo("estado", false, &[Regla::En(&["activo", "inactivo"])]),
    campo("pagina", false, &[Regla::Entero, Regla::Min(1.0)]),
    campo(
        "limite",
        false,
        &[Regla::Entero, Regla::Min(1.0), Regla::Max(100.0)],
    ),
];

/// `GET /api/v2/catalog/promociones`.
pub async fn listar_promociones(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    validar_query(LISTAR_PROMOCIONES, &query)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .operaciones()?
        .listar_promociones(&empresa, &json!(query))
        .await?)
}

/// `POST /api/v2/catalog/promociones`.
pub async fn crear_promocion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    validar(PROMOCION, &cuerpo)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    created(
        state
            .operaciones()?
            .crear_promocion(&empresa, &cuerpo)
            .await?,
    )
}

/// `GET /api/v2/catalog/promociones/:id`.
pub async fn detalle_promocion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    let promocion = state
        .operaciones()?
        .obtener_promocion(&id, &empresa)
        .await?
        .ok_or_else(|| ApiError::no_encontrado("Promoción", &id))?;
    ok(promocion)
}

/// `PATCH /api/v2/catalog/promociones/:id`.
pub async fn actualizar_promocion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    validar(PROMOCION_ACTUALIZAR, &cuerpo)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .operaciones()?
        .actualizar_promocion(&id, &empresa, &cuerpo)
        .await?)
}

/// `DELETE /api/v2/catalog/promociones/:id`.
pub async fn eliminar_promocion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_ITEMS).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    state
        .operaciones()?
        .eliminar_promocion(&id, &empresa)
        .await?;
    sin_contenido()
}

const LISTAR_MOVIMIENTOS: &[ReglaCampo] = &[
    campo("empresa_id", false, &[Regla::Uuid]),
    campo(
        "tipo",
        false,
        &[Regla::En(&["entrada", "salida", "ajuste", "traslado"])],
    ),
    campo("pagina", false, &[Regla::Numero, Regla::Min(1.0)]),
    campo(
        "limite",
        false,
        &[Regla::Numero, Regla::Min(1.0), Regla::Max(100.0)],
    ),
];

/// `GET /api/v2/catalog/inventario/movimientos`.
pub async fn listar_movimientos(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_STOCK).await?;
    validar_query(LISTAR_MOVIMIENTOS, &query)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(state
        .operaciones()?
        .movimientos(&empresa, &json!(query))
        .await?)
}

/// `POST /api/v2/catalog/inventario/movimientos`.
pub async fn crear_movimiento(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_CATALOGO, &[], CAP_STOCK).await?;
    validar(MOVIMIENTO, &cuerpo)?;
    validar_lineas_movimiento(cuerpo.get("lineas").unwrap_or(&Value::Null))?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    created(
        state
            .operaciones()?
            .crear_movimiento(&empresa, &cuerpo, Some(&usuario.user_id))
            .await?,
    )
}
