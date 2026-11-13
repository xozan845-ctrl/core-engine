//! Router, middleware de borde (seguridad, CORS, diagnóstico y rate limit).

use crate::config::Config;
use crate::diagnostics;
use crate::errors::ApiError;
use crate::metrics::Metrics;
use crate::rate_limit::{es_login, es_media_publica, ip_de, ResultadoLimite};
use crate::readiness;
use crate::state::AppState;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use std::time::{Duration, Instant};
use uuid::Uuid;

const OPENAPI: &str = include_str!("../../../../docs/api/core-engine.openapi.json");

const DOCS_HTML: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8" />
    <title>Core Engine · API</title>
    <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css" />
  </head>
  <body>
    <div id="swagger-ui"></div>
    <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>window.ui = SwaggerUIBundle({ url: '/docs-json', dom_id: '#swagger-ui' });</script>
  </body>
</html>
"#;

pub fn build_router(state: AppState) -> Router {
    let router = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/ready", get(ready))
        .route("/docs", get(docs))
        .route("/docs-json", get(docs_json))
        .route("/docs-yaml", get(docs_yaml))
        .fallback(pasarela)
        .with_state(state.clone());
    router.layer(middleware::from_fn_with_state(state, borde))
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let salud = state.metrics.salud();
    Json(json!({
        "api_gateway": "ok",
        "servicio": salud.servicio,
        "uptime_seg": salud.uptime_seg,
    }))
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

async fn ready(State(state): State<AppState>) -> Response {
    let ahora = Instant::now();
    let cacheado = {
        let cache = state.ready_cache.lock().expect("ready cache");
        cache
            .as_ref()
            .filter(|(expira, _)| *expira > ahora)
            .map(|(_, estado)| estado.clone())
    };
    let estado = match cacheado {
        Some(estado) => estado,
        None => {
            let estado = readiness::evaluar(&state.config, 1500).await;
            *state.ready_cache.lock().expect("ready cache") = Some((
                Instant::now() + Duration::from_millis(state.config.ready_cache_ms),
                estado.clone(),
            ));
            estado
        }
    };
    let codigo = StatusCode::from_u16(readiness::codigo_readiness(&estado))
        .unwrap_or(StatusCode::SERVICE_UNAVAILABLE);
    (codigo, Json(estado)).into_response()
}

async fn docs() -> Html<&'static str> {
    Html(DOCS_HTML)
}

async fn docs_json() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        OPENAPI,
    )
}

/// YAML 1.2 es un superconjunto de JSON: se sirve el mismo contrato con el
/// `content-type` de YAML para conservar la ruta `/docs-yaml` (ADR-0005) sin
/// añadir un serializador al binario.
async fn docs_yaml() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/yaml; charset=utf-8")],
        OPENAPI,
    )
}

async fn pasarela(State(state): State<AppState>, request: Request) -> Response {
    let ruta = request.uri().path().to_string();
    if ruta.starts_with("/api/v2/") {
        return crate::proxy::reenviar(State(state), request).await;
    }
    ApiError::nueva(
        "NO_ENCONTRADO",
        format!("No existe ruta {} {}.", request.method(), ruta),
        StatusCode::NOT_FOUND,
    )
    .into_response()
}

/// Middleware de borde: CORS, seguridad, diagnóstico, rate limit, correlación y
/// métricas. No modifica el cuerpo de las respuestas proxificadas.
async fn borde(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let metodo = request.method().clone();
    let ruta = request.uri().path().to_string();
    let origen = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|valor| valor.to_str().ok())
        .map(str::to_string);

    if metodo == Method::OPTIONS {
        let mut response = Response::new(Body::empty());
        *response.status_mut() = StatusCode::NO_CONTENT;
        aplicar_cors(response.headers_mut(), &state.config, origen.as_deref());
        aplicar_seguridad(response.headers_mut());
        return response;
    }

    if !diagnostics::autorizado(request.headers(), &ruta, &state.config) {
        let mut response = ApiError::nueva(
            "NO_ENCONTRADO",
            format!("No existe ruta {} {}.", metodo, ruta),
            StatusCode::NOT_FOUND,
        )
        .into_response();
        aplicar_cors(response.headers_mut(), &state.config, origen.as_deref());
        aplicar_seguridad(response.headers_mut());
        return response;
    }

    if !es_media_publica(metodo.as_str(), &ruta) {
        let ip = ip_de(request.headers());
        let (clave, limite) = if ruta == "/ready" {
            ("ready", 10)
        } else if es_login(metodo.as_str(), &ruta) {
            ("login", 10)
        } else {
            ("global", 300)
        };
        match state
            .rate
            .consumir(&ip, clave, limite, Duration::from_secs(60))
        {
            ResultadoLimite::Permitido { restantes } => {
                if let Ok(valor) = HeaderValue::from_str(&limite.to_string()) {
                    request.headers_mut().insert("x-ratelimit-limit", valor);
                }
                if let Ok(valor) = HeaderValue::from_str(&restantes.to_string()) {
                    request.headers_mut().insert("x-ratelimit-remaining", valor);
                }
            }
            ResultadoLimite::Excedido {
                reintento_seg,
                limite,
            } => {
                let mut response = ApiError::nueva(
                    "DEMASIADAS_PETICIONES",
                    "Se excedio el limite de peticiones; intente de nuevo en unos segundos.",
                    StatusCode::TOO_MANY_REQUESTS,
                )
                .con_detalles(json!({
                    "limite": limite,
                    "ventana_seg": 60,
                    "reintente_en_seg": reintento_seg,
                }))
                .into_response();
                response.headers_mut().insert(
                    header::RETRY_AFTER,
                    HeaderValue::from_str(&reintento_seg.to_string())
                        .unwrap_or_else(|_| HeaderValue::from_static("1")),
                );
                response
                    .headers_mut()
                    .insert("x-ratelimit-remaining", HeaderValue::from_static("0"));
                aplicar_cors(response.headers_mut(), &state.config, origen.as_deref());
                aplicar_seguridad(response.headers_mut());
                return response;
            }
        }
    }

    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|valor| valor.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    if let Ok(valor) = HeaderValue::from_str(&request_id) {
        request.headers_mut().insert("x-request-id", valor);
    }
    let inicio = Instant::now();
    let mut response = next.run(request).await;
    state.metrics.registrar_peticion(
        metodo.as_str(),
        &ruta,
        response.status().as_u16(),
        inicio.elapsed().as_millis() as u64,
    );
    if let Ok(valor) = HeaderValue::from_str(&request_id) {
        response.headers_mut().insert("x-request-id", valor);
    }
    aplicar_cors(response.headers_mut(), &state.config, origen.as_deref());
    aplicar_seguridad(response.headers_mut());
    response
}

fn aplicar_seguridad(headers: &mut HeaderMap) {
    let valores = [
        ("x-content-type-options", "nosniff"),
        ("x-frame-options", "DENY"),
        ("referrer-policy", "no-referrer"),
        (
            "content-security-policy",
            "default-src 'none'; frame-ancestors 'none'",
        ),
        (
            "strict-transport-security",
            "max-age=31536000; includeSubDomains",
        ),
        (
            "permissions-policy",
            "camera=(), microphone=(), geolocation=(), browsing-topics=()",
        ),
        ("cross-origin-opener-policy", "same-origin"),
        ("cross-origin-embedder-policy", "require-corp"),
        ("cross-origin-resource-policy", "same-site"),
        ("x-permitted-cross-domain-policies", "none"),
    ];
    for (clave, valor) in valores {
        headers.insert(
            axum::http::header::HeaderName::from_static(clave),
            HeaderValue::from_static(valor),
        );
    }
}

fn aplicar_cors(headers: &mut HeaderMap, config: &Config, origen: Option<&str>) {
    let Some(origen) = origen else {
        return;
    };
    if !config
        .cors_origins
        .iter()
        .any(|permitido| permitido == origen)
    {
        return;
    }
    if let Ok(valor) = HeaderValue::from_str(origen) {
        headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, valor);
    }
    headers.insert(header::VARY, HeaderValue::from_static("Origin"));
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, PATCH, PUT, DELETE, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("Content-Type, Authorization, x-request-id"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("x-request-id"),
    );
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
    use tower::ServiceExt;

    fn config() -> Config {
        Config::from_pairs([("SERVICIO", "api-gateway"), ("INTERNAL_API_KEY", "k")])
            .expect("config")
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
    async fn debe_responder_health_y_metrics() {
        let app = build_router(estado_de_prueba(config()));
        let health = app
            .clone()
            .oneshot(
                HttpRequest::get("/health")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(health.status(), StatusCode::OK);
        assert!(cuerpo(health).await.contains("api_gateway"));

        let metrics = app
            .oneshot(
                HttpRequest::get("/metrics")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(metrics.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn debe_cerrar_docs_sin_clave_y_abrirlo_con_clave() {
        let app = build_router(estado_de_prueba(config()));
        let oculto = app
            .clone()
            .oneshot(
                HttpRequest::get("/docs")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(oculto.status(), StatusCode::NOT_FOUND);

        let abierto = app
            .oneshot(
                HttpRequest::get("/docs")
                    .header("x-internal-key", "k")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(abierto.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn debe_limitar_ready_sin_clave_y_servir_con_clave() {
        let app = build_router(estado_de_prueba(config()));
        let oculto = app
            .clone()
            .oneshot(
                HttpRequest::get("/ready")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(oculto.status(), StatusCode::NOT_FOUND);

        let con_clave = app
            .oneshot(
                HttpRequest::get("/ready")
                    .header("x-internal-key", "k")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert!(matches!(
            con_clave.status(),
            StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
        ));
    }
}
