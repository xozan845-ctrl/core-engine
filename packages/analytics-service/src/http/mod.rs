//! Router HTTP y middlewares transversales.
//!
//! Capas en el mismo orden que los interceptores globales de Nest:
//! correlación (más externa) → métricas → envelope → handler.

pub mod analitica;
pub mod envelope;
pub mod interno;

use crate::auth::UsuarioContexto;
use crate::campos;
use crate::config::Config;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::metrics::Metrics;
use crate::state::AppState;
use axum::extract::{MatchedPath, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use std::time::Instant;
use uuid::Uuid;

/// Documento OpenAPI público versionado (golden del contrato, R-C-7).
const OPENAPI_PUBLICO: &str =
    include_str!("../../../../docs/openapi/analytics-service/public.openapi.json");

const DOCS_HTML: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8" />
    <title>Core Engine · analytics-service</title>
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
        .merge(analitica::rutas())
        .merge(interno::rutas())
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
        negocio_id: usuario.as_ref().and_then(|u| u.negocio_id.clone()),
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
    use crate::auth::{CapacidadActiva, Capacidades};
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn config() -> Config {
        Config::from_pairs([("SERVICIO", "analytics-service")]).expect("config")
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

    struct CapsFijas(bool);

    #[async_trait]
    impl Capacidades for CapsFijas {
        async fn activas(
            &self,
            _empresa_id: &str,
            _requeridas: &[&str],
        ) -> Result<Vec<CapacidadActiva>, ()> {
            Ok(vec![CapacidadActiva {
                empresa_id: "e-1".into(),
                capacidad_codigo: "analitica_negocios".into(),
                activo: self.0,
            }])
        }
    }

    fn estado_con_caps(activa: bool) -> AppState {
        let config = config();
        AppState {
            config: Arc::new(config.clone()),
            metrics: Arc::new(Metrics::nueva(config.servicio.clone())),
            capacidades: Arc::new(CapsFijas(activa)),
            db: None,
            inicio: std::time::Instant::now(),
        }
    }

    fn headers(rol: &str, tenant: Option<&str>) -> Vec<(&'static str, String)> {
        let mut valores = vec![
            ("x-user-id", "u-1".to_string()),
            ("x-user-rol", rol.to_string()),
        ];
        if let Some(tenant) = tenant {
            valores.push(("x-tenant", tenant.to_string()));
        }
        valores
    }

    async fn pedir(
        app: axum::Router,
        metodo: &str,
        ruta: &str,
        valores: Vec<(&'static str, String)>,
    ) -> Response {
        let mut constructor = HttpRequest::builder().method(metodo).uri(ruta);
        for (clave, valor) in valores {
            constructor = constructor.header(clave, valor);
        }
        app.oneshot(constructor.body(Body::empty()).expect("request"))
            .await
            .expect("response")
    }

    #[tokio::test]
    async fn debe_proteger_las_rutas_analiticas() {
        let app = build_router(estado_con_caps(true));
        let response = pedir(app, "GET", "/api/v2/analytics/negocios", vec![]).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn debe_exigir_tenant_y_capacidad_al_empresario() {
        let app = build_router(estado_con_caps(true));
        let response = pedir(
            app.clone(),
            "GET",
            "/api/v2/analytics/negocios",
            headers("empresario", None),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let app = build_router(estado_con_caps(true));
        let response = pedir(
            app,
            "GET",
            "/api/v2/analytics/negocios",
            headers("empresario", Some("e-1")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let app = build_router(estado_con_caps(false));
        let response = pedir(
            app,
            "GET",
            "/api/v2/analytics/negocios",
            headers("empresario", Some("e-1")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn debe_reservar_la_plataforma_a_los_administradores() {
        let app = build_router(estado_con_caps(true));
        let response = pedir(
            app,
            "GET",
            "/api/v2/analytics/plataforma",
            headers("empresario", Some("e-1")),
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn debe_validar_un_reporte_antes_de_la_base() {
        let app = build_router(estado_con_caps(true));
        let mut constructor = HttpRequest::post(
            "/api/v2/analytics/reportes?empresaId=11111111-1111-1111-1111-111111111111",
        )
        .header("content-type", "application/json");
        for (clave, valor) in headers("admin", None) {
            constructor = constructor.header(clave, valor);
        }
        let response = app
            .oneshot(
                constructor
                    .body(Body::from(
                        r#"{"nombre":"x","tipo":"otro","proxima_ejecucion":"2026-01-01T06:00:00Z"}"#,
                    ))
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "VALIDACION");
    }
}
