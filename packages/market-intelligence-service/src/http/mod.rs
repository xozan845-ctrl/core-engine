//! Router HTTP y middlewares transversales (correlación → métricas → envelope).

pub mod envelope;
pub mod inteligencia;

use crate::auth::UsuarioContexto;
use crate::campos;
use crate::config::Config;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::metrics::Metrics;
use crate::state::AppState;
use axum::extract::{FromRequest, MatchedPath, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;

/// Documento OpenAPI público versionado (golden del contrato, R-C-7).
const OPENAPI_PUBLICO: &str =
    include_str!("../../../../docs/openapi/market-intelligence-service/public.openapi.json");

const DOCS_HTML: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8" />
    <title>Core Engine · market-intelligence-service</title>
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
        .route(
            "/api/v2/inteligencia/mapa-calor",
            get(inteligencia::mapa_calor),
        )
        .route(
            "/api/v2/inteligencia/me/mapa-calor",
            get(inteligencia::mi_mapa_calor),
        )
        .route(
            "/api/v2/inteligencia/rendimiento",
            get(inteligencia::rendimiento),
        )
        .route(
            "/api/v2/inteligencia/me/rendimiento",
            get(inteligencia::mi_rendimiento),
        )
        .route(
            "/api/v2/inteligencia/cobertura",
            get(inteligencia::cobertura),
        )
        .route("/api/v2/inteligencia/demanda", get(inteligencia::demanda))
        .route(
            "/api/v2/inteligencia/tendencias",
            get(inteligencia::tendencias),
        )
        .route("/api/v2/inteligencia/resumen", get(inteligencia::resumen))
        .route(
            "/api/v2/inteligencia/ventas/registrar",
            post(inteligencia::registrar_venta),
        )
        .route(
            "/api/v2/inteligencia/forecast/demanda",
            get(inteligencia::forecast),
        )
        .route(
            "/api/v2/inteligencia/anomalias",
            get(inteligencia::anomalias),
        )
        .route(
            "/api/v2/inteligencia/vendedores/{id}/score",
            get(inteligencia::score_vendedor),
        )
        .route("/api/v2/inteligencia/me/score", get(inteligencia::mi_score))
        .route(
            "/api/v2/inteligencia/calidad",
            get(inteligencia::calidad_datos),
        )
        .fallback(no_encontrado)
        .with_state(state.clone());
    aplicar_capas(router, state)
}

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
    crate::errors::ApiError::nueva(
        "NO_ENCONTRADO",
        "Ruta no encontrada.",
        StatusCode::NOT_FOUND,
    )
    .into_response()
}

pub struct Cuerpo(pub Value);

impl<S> FromRequest<S> for Cuerpo
where
    S: Send + Sync,
{
    type Rejection = crate::errors::ApiError;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::to_bytes(request.into_body(), 100 * 1024)
            .await
            .map_err(|_| {
                crate::errors::ApiError::nueva(
                    "HTTP_413",
                    "El cuerpo de la peticion excede el limite permitido.",
                    StatusCode::PAYLOAD_TOO_LARGE,
                )
            })?;
        if bytes.is_empty() {
            return Ok(Cuerpo(Value::Null));
        }
        serde_json::from_slice(&bytes).map(Cuerpo).map_err(|_| {
            crate::errors::ApiError::nueva(
                "SOLICITUD_INVALIDA",
                "Peticion invalida.",
                StatusCode::BAD_REQUEST,
            )
        })
    }
}

async fn correlacion(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|valor| valor.to_str().ok())
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let usuario = UsuarioContexto::desde_headers(request.headers());
    let usuario_id = usuario.as_ref().map(|u| u.user_id.clone()).or_else(|| {
        request
            .headers()
            .get("x-user-personal")
            .and_then(|valor| valor.to_str().ok())
            .map(str::to_string)
    });
    let metodo = request.method().as_str().to_string();
    let ruta = request
        .uri()
        .path_and_query()
        .map(|valor| valor.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    let contexto = ContextoCorrelacion {
        request_id: request_id.clone(),
        servicio: state.config.servicio.clone(),
        usuario_id: usuario_id.clone(),
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

pub fn estado_de_prueba(config: Config) -> AppState {
    AppState::nueva(config.clone(), Metrics::nueva(config.servicio.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use serde_json::json;
    use tower::ServiceExt;

    fn config() -> Config {
        Config::from_pairs([("SERVICIO", "market-intelligence-service")]).expect("config")
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
        assert_eq!(valor["servicio"], "market-intelligence-service");
        assert!(valor.get("data").is_none());
    }

    #[tokio::test]
    async fn debe_responder_404_estructurado() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/inteligencia/desconocido")
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
    async fn debe_exigir_contexto_en_las_rutas_de_negocio() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/inteligencia/resumen")
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
        assert_eq!(valor["x-servicio"], "market-intelligence-service");
    }

    #[tokio::test]
    async fn debe_envolver_las_rutas_publicas() {
        let state = estado_de_prueba(config());
        let app = aplicar_capas(
            Router::new()
                .route(
                    "/api/v2/inteligencia/prueba",
                    get(|| async { Json(json!({ "ok": true })) }),
                )
                .with_state(state.clone()),
            state,
        );
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/inteligencia/prueba")
                    .header("x-request-id", "req-42")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.headers()["x-request-id"], "req-42");
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["data"]["ok"], true);
        assert_eq!(valor["meta"]["statusCode"], 200);
    }
}
