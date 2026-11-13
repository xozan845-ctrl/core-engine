//! Router HTTP y middlewares transversales.
//!
//! Misma composición de capas que el servicio TypeScript: correlación (más
//! externa) → métricas → envelope → handler.

pub mod comercio;
pub mod envelope;
pub mod finanzas;
pub mod interno;

use crate::auth::UsuarioContexto;
use crate::campos;
use crate::config::Config;
use crate::errors::ApiError;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::metrics::Metrics;
use crate::state::AppState;
use axum::extract::{FromRequest, MatchedPath, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

/// Documento OpenAPI público versionado (golden del contrato, R-C-7).
const OPENAPI_PUBLICO: &str =
    include_str!("../../../../docs/openapi/finance-service/public.openapi.json");

const DOCS_HTML: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8" />
    <title>Core Engine · finance-service</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
  </head>
  <body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>
      window.ui = SwaggerUIBundle({ url: '/docs-json', dom_id: '#swagger-ui' });
    </script>
  </body>
</html>
"#;

pub fn build_router(state: AppState) -> Router {
    let router = crate::rutas::registrar(Router::new())
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/docs", get(docs))
        .route("/docs-json", get(docs_json))
        .fallback(no_encontrado)
        .with_state(state.clone());
    aplicar_capas(router, state)
}

/// Capas en el mismo orden que los interceptores globales de Nest.
pub(crate) fn aplicar_capas(router: Router, state: AppState) -> Router {
    router
        .layer(middleware::from_fn(envelope::envelope))
        .layer(middleware::from_fn_with_state(state.clone(), metricas))
        .layer(middleware::from_fn_with_state(state, correlacion))
}

async fn health(State(state): State<AppState>) -> Json<crate::metrics::MetricaSalud> {
    Json(state.metrics.salud())
}

async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        state.metrics.texto(),
    )
}

async fn docs() -> Html<&'static str> {
    Html(DOCS_HTML)
}

async fn docs_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        OPENAPI_PUBLICO,
    )
}

async fn no_encontrado() -> Response {
    ApiError::nueva(
        "NO_ENCONTRADO",
        "Ruta no encontrada.",
        StatusCode::NOT_FOUND,
    )
    .into_response()
}

/// Cuerpo JSON con el shape de error de Nest cuando el payload es inválido.
/// Un cuerpo vacío se trata como `null` para que la validación DTO reporte los
/// campos requeridos (igual que el body-parser de Express).
pub struct Cuerpo(pub Value);

impl<S> FromRequest<S> for Cuerpo
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::to_bytes(request.into_body(), 1024 * 1024)
            .await
            .map_err(|_| {
                ApiError::nueva(
                    "HTTP_413",
                    "El cuerpo de la peticion excede el limite permitido.",
                    StatusCode::PAYLOAD_TOO_LARGE,
                )
            })?;
        if bytes.is_empty() {
            return Ok(Cuerpo(Value::Null));
        }
        serde_json::from_slice(&bytes).map(Cuerpo).map_err(|_| {
            ApiError::nueva(
                "SOLICITUD_INVALIDA",
                "Peticion invalida.",
                StatusCode::BAD_REQUEST,
            )
        })
    }
}

/// Contexto + roles + capacidades de una ruta pública (equivalente a
/// `RolesGuard` + `CapabilityGuard`).
pub async fn contexto(
    state: &AppState,
    headers: &HeaderMap,
    roles: &[&str],
    permisos: &[&str],
    capacidades: &[&str],
) -> Result<UsuarioContexto, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(headers)?;
    usuario.exigir_roles(roles, permisos)?;
    state.exigir_capacidades(&usuario, capacidades).await?;
    Ok(usuario)
}

/// `ClaveInternaGuard`: fail-closed sobre `INTERNAL_API_KEY`.
pub fn exigir_interna(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let esperada = state.config.internal_api_key.as_deref().unwrap_or_default();
    let recibida = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or_default();
    if esperada.is_empty() || recibida != esperada {
        return Err(ApiError::acceso_denegado("Clave interna invalida."));
    }
    Ok(())
}

/// Convierte un query ISO 8601 a `DateTime<Utc>` (400 si es inválido).
pub fn fecha_query(valor: Option<&String>) -> Result<Option<DateTime<Utc>>, ApiError> {
    let Some(valor) = valor
        .map(|valor| valor.trim())
        .filter(|valor| !valor.is_empty())
    else {
        return Ok(None);
    };
    DateTime::parse_from_rfc3339(valor)
        .map(|fecha| Some(fecha.with_timezone(&Utc)))
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(valor, "%Y-%m-%d")
                .map(|fecha| {
                    Some(DateTime::<Utc>::from_naive_utc_and_offset(
                        fecha.and_hms_opt(0, 0, 0).unwrap_or_default(),
                        Utc,
                    ))
                })
                .map_err(|_| {
                    ApiError::validacion(serde_json::json!([
                        {"campo": "fecha", "restricciones": ["isDateString"]}
                    ]))
                })
        })
}

pub fn query<'a>(query: &'a HashMap<String, String>, clave: &str) -> Option<&'a String> {
    query.get(clave).filter(|valor| !valor.trim().is_empty())
}

pub fn query_i64(query: &HashMap<String, String>, clave: &str, defecto: i64) -> i64 {
    query
        .get(clave)
        .and_then(|valor| valor.trim().parse::<i64>().ok())
        .unwrap_or(defecto)
}

pub fn query_f64(query: &HashMap<String, String>, clave: &str, defecto: f64) -> f64 {
    query
        .get(clave)
        .and_then(|valor| valor.trim().parse::<f64>().ok())
        .unwrap_or(defecto)
}

/// ID de correlación, logs de petición y contexto para RLS.
async fn correlacion(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|valor| valor.to_str().ok())
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let usuario = UsuarioContexto::desde_headers(request.headers());
    let usuario_id = usuario.as_ref().map(|u| u.user_id.clone());
    let metodo = request.method().as_str().to_string();
    let ruta = request
        .uri()
        .path_and_query()
        .map(|valor| valor.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let contexto = ContextoCorrelacion {
        request_id: request_id.clone(),
        servicio: state.config.servicio.clone(),
        usuario_id: usuario.as_ref().map(|u| u.user_id.clone()),
        rol: usuario.as_ref().map(|u| u.rol.clone()),
        tenant_id: usuario.as_ref().and_then(|u| u.tenant_id.clone()),
    };

    let mut response = ejecutar_con_contexto(contexto, async move {
        crate::logs::info(
            "http",
            campos! {
                "msg": "peticion",
                "metodo": metodo,
                "ruta": ruta,
                "usuario": usuario_id,
            },
        );
        next.run(request).await
    })
    .await;
    if let Ok(valor) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", valor);
    }
    response
}

/// Latencia, totales y errores por ruta (mismos nombres que `MetricsService`).
async fn metricas(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let metodo = request.method().as_str().to_string();
    let plantilla = request
        .extensions()
        .get::<MatchedPath>()
        .map(|ruta| ruta.as_str().to_string());
    let inicio = Instant::now();
    let response = next.run(request).await;
    let ruta = plantilla.unwrap_or_else(|| "/_unmatched".to_string());
    state.metrics.registrar_peticion(
        &metodo,
        &ruta,
        response.status().as_u16(),
        inicio.elapsed().as_millis() as u64,
    );
    response
}

/// Construye el estado de pruebas sin depender de infraestructura.
pub fn estado_de_prueba(config: Config) -> AppState {
    AppState::nueva(config.clone(), Metrics::nueva(config.servicio.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn config() -> Config {
        Config::from_pairs([("SERVICIO", "finance-service")]).expect("config")
    }

    async fn cuerpo(response: Response) -> String {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("cuerpo")
            .to_bytes();
        String::from_utf8(bytes.to_vec()).expect("utf8")
    }

    #[tokio::test]
    async fn debe_responder_salud_sin_envelope() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["servicio"], "finance-service");
        assert!(valor["uptime_seg"].is_number());
        assert!(valor.get("data").is_none());
    }

    #[tokio::test]
    async fn debe_exponer_metricas_en_texto_prometheus() {
        let state = estado_de_prueba(config());
        let app = build_router(state);
        let _ = app
            .clone()
            .oneshot(
                HttpRequest::get("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("health");
        let response = app
            .oneshot(
                HttpRequest::get("/metrics")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("metrics");
        assert_eq!(response.status(), StatusCode::OK);
        let texto = cuerpo(response).await;
        assert!(texto.contains("core_engine_http_requests_total"));
        assert!(texto.contains("ruta=\"/health\""));
    }

    #[tokio::test]
    async fn debe_responder_404_estructurado_en_rutas_desconocidas() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/finanzas/desconocido")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "NO_ENCONTRADO");
    }

    #[tokio::test]
    async fn debe_envolver_las_rutas_publicas_y_propagar_request_id() {
        let state = estado_de_prueba(config());
        let app = aplicar_capas(
            Router::new()
                .route(
                    "/api/v2/finanzas/prueba",
                    get(|| async { Json(serde_json::json!({ "ok": true })) }),
                )
                .with_state(state.clone()),
            state,
        );
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/finanzas/prueba")
                    .header("x-request-id", "req-42")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.headers()["x-request-id"], "req-42");
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["data"]["ok"], true);
        assert_eq!(valor["meta"]["path"], "/api/v2/finanzas/prueba");
        assert_eq!(valor["meta"]["statusCode"], 200);
    }

    #[tokio::test]
    async fn debe_servir_el_openapi_versionado() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/docs-json")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["x-servicio"], "finance-service");
    }

    #[tokio::test]
    async fn debe_exigir_contexto_en_las_rutas_de_negocio() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/finanzas/cuentas")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "NO_AUTORIZADO");
    }

    #[tokio::test]
    async fn debe_rechazar_la_clave_interna_incorrecta() {
        let mut params = vec![("SERVICIO", "finance-service")];
        params.push(("INTERNAL_API_KEY", "secreto"));
        let config = Config::from_pairs(params).expect("config");
        let app = build_router(estado_de_prueba(config));
        let response = app
            .oneshot(
                HttpRequest::get("/internal/finance/sujetos/u-1")
                    .header("x-internal-key", "otra")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    fn headers_usuario(rol: &str, tenant: Option<&str>) -> Vec<(&'static str, String)> {
        let mut headers = vec![
            (
                "x-user-id",
                "11111111-1111-4111-8111-111111111111".to_string(),
            ),
            ("x-user-rol", rol.to_string()),
        ];
        if let Some(tenant) = tenant {
            headers.push(("x-tenant", tenant.to_string()));
        }
        headers
    }

    #[tokio::test]
    async fn debe_calcular_el_punto_de_equilibrio_sin_base_de_datos() {
        let app = build_router(estado_de_prueba(config()));
        let mut request = HttpRequest::get("/api/v2/finanzas/punto-equilibrio");
        for (clave, valor) in headers_usuario("admin", None) {
            request = request.header(clave, valor);
        }
        let response = app
            .oneshot(request.body(Body::empty()).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert!(valor["data"]["punto_equilibrio_caja_cents"].is_number());
        assert_eq!(valor["meta"]["statusCode"], 200);
    }

    #[tokio::test]
    async fn debe_exponer_los_estudios_del_modelo() {
        for ruta in [
            "/api/v2/finanzas/proyecciones/sensibilidad",
            "/api/v2/finanzas/proyecciones/plan-bienal",
        ] {
            let app = build_router(estado_de_prueba(config()));
            let mut request = HttpRequest::get(ruta);
            for (clave, valor) in headers_usuario("admin", None) {
                request = request.header(clave, valor);
            }
            let response = app
                .oneshot(request.body(Body::empty()).expect("request"))
                .await
                .expect("response");
            assert_eq!(response.status(), StatusCode::OK, "ruta {ruta}");
        }
    }

    #[tokio::test]
    async fn debe_denegar_el_modulo_financiero_a_roles_no_autorizados() {
        let app = build_router(estado_de_prueba(config()));
        let mut request = HttpRequest::get("/api/v2/finanzas/cuentas");
        for (clave, valor) in headers_usuario("vendedor", None) {
            request = request.header(clave, valor);
        }
        let response = app
            .oneshot(request.body(Body::empty()).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "ACCESO_DENEGADO");
    }

    #[tokio::test]
    async fn debe_validar_el_cuerpo_antes_de_tocar_la_base() {
        let app = build_router(estado_de_prueba(config()));
        let mut request = HttpRequest::post("/api/v2/finanzas/asientos")
            .header("content-type", "application/json");
        for (clave, valor) in headers_usuario("admin", None) {
            request = request.header(clave, valor);
        }
        let response = app
            .oneshot(
                request
                    .body(Body::from(r#"{"concepto":"x","detalles":[]}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "VALIDACION");
    }

    #[tokio::test]
    async fn debe_rechazar_un_cuerpo_no_json() {
        let app = build_router(estado_de_prueba(config()));
        let mut request =
            HttpRequest::post("/api/v2/comercio/gastos").header("content-type", "application/json");
        for (clave, valor) in headers_usuario("empresario", Some("t-1")) {
            request = request.header(clave, valor);
        }
        let response = app
            .oneshot(request.body(Body::from("no-json")).expect("request"))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "SOLICITUD_INVALIDA");
    }

    #[tokio::test]
    async fn debe_validar_la_clave_interna_antes_del_cuerpo() {
        let config = Config::from_pairs([
            ("SERVICIO", "finance-service"),
            ("INTERNAL_API_KEY", "secreto"),
        ])
        .expect("config");
        let app = build_router(estado_de_prueba(config));
        let response = app
            .oneshot(
                HttpRequest::post("/internal/finance/creditos")
                    .header("x-internal-key", "secreto")
                    .body(Body::from(r#"{"empresa_id":"no-uuid"}"#))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "VALIDACION");
    }

    #[test]
    fn debe_parsear_fechas_iso_y_cortas() {
        assert!(fecha_query(None).expect("vacio").is_none());
        assert!(fecha_query(Some(&"2026-11-13".to_string()))
            .expect("fecha")
            .is_some());
        assert!(fecha_query(Some(&"2026-11-13T10:00:00Z".to_string()))
            .expect("fecha")
            .is_some());
        assert!(fecha_query(Some(&"no-fecha".to_string())).is_err());
    }

    #[test]
    fn debe_leer_querys_con_defaults() {
        let mut query = HashMap::new();
        query.insert("limite".to_string(), "25".to_string());
        assert_eq!(query_i64(&query, "limite", 200), 25);
        assert_eq!(query_f64(&query, "tasa", 0.12), 0.12);
        assert_eq!(query_i64(&query, "otro", 7), 7);
    }
}
