//! Handlers de inventario (`/api/v2/admin/inventario`).

use super::validacion::{self, campo, Regla, ReglaCampo};
use super::{autorizar, Cuerpo};
use crate::auth::{UsuarioContexto, ROLES_INVENTARIO, ROLES_INVENTARIO_ADMIN};
use crate::errors::ApiError;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::Value;
use std::collections::HashMap;

const CAP_STOCK: &[&str] = &["inventario_stock"];
const CAP_LOTES: &[&str] = &["inventario_lotes"];

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

const CREAR_PROVEEDOR: &[ReglaCampo] = &[
    campo("empresa_id", true, &[Regla::Uuid]),
    campo("negocio_id", false, &[Regla::Uuid]),
    campo("codigo", true, &[Regla::Texto]),
    campo("nombre", true, &[Regla::Texto]),
    campo("correo", false, &[Regla::Texto]),
    campo("telefono", false, &[Regla::Texto]),
];

const CREAR_LOTE: &[ReglaCampo] = &[
    campo("empresa_id", true, &[Regla::Uuid]),
    campo("item_id", true, &[Regla::Uuid]),
    campo("proveedor_id", false, &[Regla::Uuid]),
    campo("codigo_lote", true, &[Regla::Texto]),
    campo("vence_en", false, &[Regla::Fecha]),
    campo("cantidad_inicial", true, &[Regla::Entero, Regla::Min(0.0)]),
];

const CONSUMIR_LOTES: &[ReglaCampo] = &[
    campo("empresa_id", true, &[Regla::Uuid]),
    campo("item_id", true, &[Regla::Uuid]),
    campo("cantidad", true, &[Regla::Entero, Regla::Min(1.0)]),
    campo("codigo_lote", false, &[Regla::Texto]),
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

fn empresa(usuario: &UsuarioContexto, solicitada: Option<&str>) -> Result<String, ApiError> {
    if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        return usuario
            .tenant_id
            .clone()
            .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."));
    }
    solicitada
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "Indica empresa_id."))
}

fn negocio(usuario: &UsuarioContexto, query: &HashMap<String, String>) -> Option<String> {
    usuario.negocio_id.clone().or_else(|| {
        query
            .get("negocio_id")
            .cloned()
            .filter(|valor| !valor.is_empty())
    })
}

/// `GET /api/v2/admin/inventario`.
pub async fn listar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_INVENTARIO_ADMIN, &[], CAP_STOCK).await?;
    let empresa = if usuario.rol == "empresario" || usuario.rol_empresa_id.is_some() {
        Some(
            usuario
                .tenant_id
                .as_deref()
                .ok_or_else(|| ApiError::dominio("EMPRESA_REQUERIDA", "No hay empresa activa."))?,
        )
    } else {
        query.get("empresa_id").map(String::as_str)
    };
    let negocio = usuario
        .negocio_id
        .as_deref()
        .or_else(|| query.get("negocio_id").map(String::as_str));
    ok(Value::Array(
        state.inventario()?.listar(empresa, negocio).await?,
    ))
}

/// `POST /api/v2/admin/inventario/proveedores`.
pub async fn crear_proveedor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(mut cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &state,
        &headers,
        ROLES_INVENTARIO_ADMIN,
        &[],
        &[CAP_LOTES[0], CAP_STOCK[0]],
    )
    .await?;
    validar(CREAR_PROVEEDOR, &cuerpo)?;
    if let Some(negocio) = usuario.negocio_id.as_ref() {
        if let Some(objeto) = cuerpo.as_object_mut() {
            objeto.insert("negocio_id".into(), Value::String(negocio.clone()));
        }
    }
    created(state.inventario()?.crear_proveedor(&cuerpo).await?)
}

/// `POST /api/v2/admin/inventario/lotes`.
pub async fn crear_lote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    autorizar(
        &state,
        &headers,
        ROLES_INVENTARIO_ADMIN,
        &[],
        &[CAP_LOTES[0], CAP_STOCK[0]],
    )
    .await?;
    validar(CREAR_LOTE, &cuerpo)?;
    created(state.inventario()?.crear_lote(&cuerpo).await?)
}

/// `POST /api/v2/admin/inventario/lotes/consumir`.
pub async fn consumir_lote(
    State(state): State<AppState>,
    headers: HeaderMap,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    autorizar(
        &state,
        &headers,
        ROLES_INVENTARIO_ADMIN,
        &[],
        &[CAP_LOTES[0], CAP_STOCK[0]],
    )
    .await?;
    validar(CONSUMIR_LOTES, &cuerpo)?;
    created(state.inventario()?.consumir_lote(&cuerpo).await?)
}

/// `GET /api/v2/admin/inventario/lotes`.
pub async fn listar_lotes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(
        &state,
        &headers,
        ROLES_INVENTARIO_ADMIN,
        &[],
        &[CAP_LOTES[0], CAP_STOCK[0]],
    )
    .await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    ok(Value::Array(
        state
            .inventario()?
            .listar_lotes(&empresa, query.get("item_id").map(String::as_str))
            .await?,
    ))
}

/// `GET /api/v2/catalog/configuracion-inventario`.
pub async fn configuracion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_INVENTARIO, &[], CAP_STOCK).await?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    let negocio = negocio(&usuario, &query);
    ok(state
        .inventario()?
        .configuracion(&empresa, negocio.as_deref())
        .await?)
}

/// `PUT /api/v2/catalog/configuracion-inventario`.
pub async fn guardar_configuracion(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
    Cuerpo(cuerpo): Cuerpo,
) -> Result<Response, ApiError> {
    let usuario = autorizar(&state, &headers, ROLES_INVENTARIO, &[], CAP_STOCK).await?;
    validar(CONFIGURACION, &cuerpo)?;
    let empresa = empresa(&usuario, query.get("empresa_id").map(String::as_str))?;
    let negocio = negocio(&usuario, &query);
    ok(state
        .inventario()?
        .guardar_configuracion(&empresa, negocio.as_deref(), &cuerpo)
        .await?)
}

#[cfg(test)]
mod tests {
    use crate::auth::{CapacidadActiva, CapacidadesFijas};
    use crate::config::Config;
    use crate::http::{build_router, estado_de_prueba};
    use axum::body::Body;
    use axum::http::{Request as HttpRequest, StatusCode};
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn config() -> Config {
        Config::from_pairs([("SERVICIO", "inventory-service")]).expect("config")
    }

    fn con_capacidades(activas: &[&str]) -> crate::state::AppState {
        let mut state = estado_de_prueba(config());
        state.capacidades = Arc::new(CapacidadesFijas(
            activas
                .iter()
                .map(|codigo| CapacidadActiva {
                    empresa_id: "t-1".to_string(),
                    capacidad_codigo: (*codigo).to_string(),
                    activo: true,
                })
                .collect(),
        ));
        state
    }

    fn headers() -> [(&'static str, &'static str); 3] {
        [
            ("x-user-id", "u-1"),
            ("x-user-rol", "admin"),
            ("x-tenant", "t-1"),
        ]
    }

    async fn cuerpo(response: axum::response::Response) -> serde_json::Value {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("cuerpo")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("json")
    }

    #[tokio::test]
    async fn debe_exigir_autenticacion_en_el_inventario() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/admin/inventario")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(cuerpo(response).await["codigo"], "NO_AUTORIZADO");
    }

    #[tokio::test]
    async fn debe_exigir_la_capacidad_de_stock() {
        let app = build_router(con_capacidades(&[]));
        let request = HttpRequest::get("/api/v2/catalog/configuracion-inventario")
            .header("x-user-id", "u-1")
            .header("x-user-rol", "empresario")
            .header("x-tenant", "t-1");
        let response = app
            .oneshot(request.body(Body::empty()).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let valor = cuerpo(response).await;
        assert_eq!(valor["codigo"], "CAPACIDAD_NO_ACTIVA");
        assert_eq!(valor["detalles"]["capacidades"][0], "inventario_stock");
    }

    #[tokio::test]
    async fn debe_validar_el_dto_antes_de_tocar_la_base() {
        let app = build_router(con_capacidades(&["inventario_stock", "inventario_lotes"]));
        let mut request = HttpRequest::post("/api/v2/admin/inventario/lotes")
            .header("content-type", "application/json");
        for (clave, valor) in headers() {
            request = request.header(clave, valor);
        }
        let response = app
            .oneshot(request.body(Body::from("{}")).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let valor = cuerpo(response).await;
        assert_eq!(valor["codigo"], "VALIDACION");
        assert_eq!(valor["detalles"][0]["campo"], "empresa_id");
    }

    #[tokio::test]
    async fn debe_validar_el_metodo_de_costeo_en_la_configuracion() {
        let app = build_router(con_capacidades(&["inventario_stock"]));
        let mut request = HttpRequest::put("/api/v2/catalog/configuracion-inventario")
            .header("content-type", "application/json");
        for (clave, valor) in headers() {
            request = request.header(clave, valor);
        }
        let response = app
            .oneshot(
                request
                    .body(Body::from(r#"{"metodo_costeo":"lifo"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let valor = cuerpo(response).await;
        assert_eq!(valor["codigo"], "VALIDACION");
        assert_eq!(valor["detalles"][0]["restricciones"][0], "isIn");
    }
}
