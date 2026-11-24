//! Router HTTP y middlewares transversales.
//!
//! Superficie: `/health`, `/metrics`, `/docs` (contrato OpenAPI versionado) y
//! la API admin de plantillas/webhooks. Los errores y la salud no llevan
//! envelope `{data, meta}`.

use crate::auth::UsuarioContexto;
use crate::campos;
use crate::config::Config;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::metrics::Metrics;
use crate::state::AppState;
use axum::extract::{MatchedPath, Path, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use std::time::Instant;
use uuid::Uuid;

mod webhooks;

/// Documentos OpenAPI versionados (golden del contrato, R-C-7).
const OPENAPI_PUBLICO: &str =
    include_str!("../../../../docs/openapi/notifications-service/public.openapi.json");
const OPENAPI_INTERNO: &str =
    include_str!("../../../../docs/openapi/notifications-service/internal.openapi.json");

const DOCS_HTML: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8" />
    <title>Core Engine · notifications-service</title>
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
    let router = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/docs", get(docs))
        .route("/docs-json", get(docs_json))
        .route("/docs-internal-json", get(docs_internal_json))
        .route(
            "/api/v2/admin/notifications/plantillas",
            get(listar_plantillas),
        )
        .route(
            "/api/v2/admin/notifications/plantillas/{evento}/{canal}",
            put(guardar_plantilla),
        )
        .route(
            "/api/v2/admin/notifications/webhooks",
            get(webhooks::listar).post(webhooks::crear),
        )
        .route(
            "/api/v2/admin/notifications/webhooks/{id}",
            patch(webhooks::actualizar).delete(webhooks::eliminar),
        )
        .route(
            "/api/v2/admin/notifications/webhooks/{id}/rotar-secreto",
            post(webhooks::rotar),
        )
        .route(
            "/api/v2/admin/notifications/webhooks/{id}/entregas",
            get(webhooks::entregas),
        )
        .route(
            "/api/v2/admin/notifications/webhooks/entregas/{id}/reintentar",
            post(webhooks::reintentar),
        )
        .route(
            "/internal/notifications/tenant/exportar",
            post(exportar_tenant),
        )
        .route("/internal/notifications/tenant/purgar", post(purgar_tenant))
        .fallback(no_encontrado)
        .with_state(state.clone());
    aplicar_capas(router, state)
}

/// Capas en el mismo orden que los interceptores globales de Nest:
/// correlación (más externa) → métricas → handler.
pub(crate) fn aplicar_capas(router: Router, state: AppState) -> Router {
    router
        .layer(middleware::from_fn_with_state(state.clone(), metricas))
        .layer(middleware::from_fn_with_state(state, correlacion))
}

fn exigir_clave(state: &AppState, headers: &axum::http::HeaderMap) -> Result<(), StatusCode> {
    let esperada = state.config.internal_api_key.as_deref().unwrap_or("");
    let recibida = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok())
        .unwrap_or("");
    if esperada.is_empty() || recibida != esperada {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(())
}

fn empresa_del_cuerpo(cuerpo: &serde_json::Value) -> Result<String, StatusCode> {
    cuerpo
        .get("empresa_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
        .map(str::to_string)
        .ok_or(StatusCode::BAD_REQUEST)
}

async fn exportar_tenant(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(cuerpo): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    exigir_clave(&state, &headers)?;
    let empresa_id = empresa_del_cuerpo(&cuerpo)?;
    let plantillas = state
        .plantillas()?
        .exportar(&empresa_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let webhooks = state
        .webhooks()?
        .exportar(&empresa_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "servicio": "notifications",
        "estado": "completado",
        "datos": { "plantillas": plantillas, "webhooks": webhooks },
    })))
}

async fn purgar_tenant(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(cuerpo): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    exigir_clave(&state, &headers)?;
    let empresa_id = empresa_del_cuerpo(&cuerpo)?;
    let plantillas = state
        .plantillas()?
        .purgar(&empresa_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let webhooks = state
        .webhooks()?
        .purgar(&empresa_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "servicio": "notifications",
        "estado": "completado",
        "detalle": { "plantillas": plantillas, "webhooks": webhooks },
    })))
}

fn envelope(data: serde_json::Value, path: &str, method: &str) -> Json<serde_json::Value> {
    envelope_con(data, path, method, 200)
}

pub(crate) fn envelope_con(
    data: serde_json::Value,
    path: &str,
    method: &str,
    status: u16,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "data": data,
        "meta": {
            "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "path": path,
            "method": method,
            "statusCode": status,
        }
    }))
}

async fn listar_plantillas(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let actor = UsuarioContexto::desde_headers(&headers)?;
    actor.exigir_roles(&["super_admin", "admin", "empresario"])?;
    let datos = state
        .plantillas()?
        .listar(actor.tenant_id.as_deref())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(envelope(
        serde_json::json!({ "plantillas": datos }),
        "/api/v2/admin/notifications/plantillas",
        "GET",
    ))
}

async fn guardar_plantilla(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path((evento, canal)): Path<(String, String)>,
    Json(cuerpo): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let actor = UsuarioContexto::desde_headers(&headers)?;
    actor.exigir_roles(&["super_admin", "admin"])?;
    let texto = cuerpo
        .get("cuerpo")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|valor| (1..=4000).contains(&valor.len()))
        .ok_or(StatusCode::BAD_REQUEST)?;
    let asunto = cuerpo
        .get("asunto")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    if asunto.chars().count() > 200 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let activo = cuerpo
        .get("activo")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    let empresa_id = cuerpo
        .get("empresa_id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| actor.tenant_id.clone());
    let plantilla = state
        .plantillas()?
        .guardar(
            empresa_id.as_deref(),
            &evento,
            &canal,
            asunto,
            texto,
            activo,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(envelope(
        serde_json::json!({
            "empresa_id": plantilla.empresa_id,
            "evento": plantilla.evento,
            "canal": plantilla.canal,
            "asunto": plantilla.asunto,
            "cuerpo": plantilla.cuerpo,
            "activo": plantilla.activo,
        }),
        &format!("/api/v2/admin/notifications/plantillas/{evento}/{canal}"),
        "PUT",
    ))
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

async fn docs_internal_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        OPENAPI_INTERNO,
    )
}

async fn no_encontrado() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "codigo": "NO_ENCONTRADO",
            "mensaje": "Ruta no encontrada.",
        })),
    )
        .into_response()
}

/// ID de correlación y logs de petición (equivalente a `TrazabilidadInterceptor`).
async fn correlacion(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|valor| valor.to_str().ok())
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let metodo = request.method().as_str().to_string();
    let ruta = request
        .uri()
        .path_and_query()
        .map(|valor| valor.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let contexto = ContextoCorrelacion {
        request_id: request_id.clone(),
        servicio: state.config.servicio.clone(),
        usuario_id: None,
        rol: None,
        tenant_id: None,
        negocio_id: None,
    };

    let mut response = ejecutar_con_contexto(contexto, async move {
        crate::logs::info(
            "http",
            campos! {
                "msg": "peticion",
                "metodo": metodo,
                "ruta": ruta,
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
        Config::from_pairs([("SERVICIO", "notifications-service")]).expect("config")
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
        let valor: serde_json::Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["servicio"], "notifications-service");
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
                HttpRequest::get("/desconocido")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let valor: serde_json::Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "NO_ENCONTRADO");
    }

    #[tokio::test]
    async fn debe_propagar_el_request_id() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/health")
                    .header("x-request-id", "req-42")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.headers()["x-request-id"], "req-42");
    }

    #[tokio::test]
    async fn debe_servir_los_openapi_versionados() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .clone()
            .oneshot(
                HttpRequest::get("/docs-json")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let valor: serde_json::Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["x-servicio"], "notifications-service");

        let response = app
            .oneshot(
                HttpRequest::get("/docs-internal-json")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        let valor: serde_json::Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["x-superficie"], "interna");
    }
}
