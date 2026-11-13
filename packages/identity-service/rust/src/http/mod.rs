//! Router HTTP y middlewares transversales.
//!
//! En el andamiaje solo viven las rutas de infraestructura (`/health`,
//! `/metrics`, `/docs`). Las rutas de identidad se añaden en los PR 2-7 con la
//! misma composición de capas (envelope, correlación y métricas).

pub mod api_keys;
pub mod auth;
pub mod capacidades;
pub mod configuracion_sitio;
pub mod empresas;
pub mod envelope;
pub mod packs;
pub mod personal;
pub mod usuarios;
pub mod validacion;

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
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;

/// Documento OpenAPI público versionado (golden del contrato, R-C-7).
const OPENAPI_PUBLICO: &str =
    include_str!("../../../../../docs/openapi/identity-service/public.openapi.json");

const DOCS_HTML: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8" />
    <title>Core Engine · identity-service</title>
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
        .route("/api/v2/auth/registro", post(auth::registrar))
        .route("/api/v2/auth/login", post(auth::login))
        .route("/api/v2/auth/refresh", post(auth::refresh))
        .route("/api/v2/auth/logout", post(auth::logout))
        .route("/api/v2/auth/crear-usuario", post(auth::crear_usuario))
        .route(
            "/api/v2/auth/cambiar-contrasena",
            post(auth::cambiar_contrasena),
        )
        .route(
            "/api/v2/auth/restablecer-contrasena",
            post(auth::restablecer_contrasena),
        )
        .route(
            "/api/v2/auth/restablecer-contrasena/confirmar",
            post(auth::confirmar_restablecimiento),
        )
        .route("/api/v2/auth/me", get(auth::me))
        .route(
            "/api/v2/api-keys",
            get(api_keys::listar).post(api_keys::crear),
        )
        .route("/api/v2/api-keys/{id}", delete(api_keys::revocar))
        .route("/api/v2/auth/token", post(api_keys::token))
        .route(
            "/api/v2/auth/vincular-personal",
            post(auth::vincular_personal),
        )
        .route("/api/v2/usuarios", get(usuarios::listar))
        .route("/api/v2/usuarios/{id}", get(usuarios::obtener))
        .route(
            "/api/v2/usuarios/{id}/avatar",
            patch(usuarios::actualizar_avatar),
        )
        .route("/api/v2/empresas", post(empresas::crear))
        .route("/api/v2/empresas/{id}", get(empresas::obtener))
        .route(
            "/api/v2/empresas/{id}/perfil",
            patch(empresas::actualizar_perfil),
        )
        .route(
            "/api/v2/empresas/{id}/pack",
            patch(empresas::actualizar_pack),
        )
        .route(
            "/api/v2/empresas/{id}/slug",
            patch(empresas::actualizar_slug),
        )
        .route(
            "/api/v2/empresas/{id}/vendedores",
            post(empresas::crear_vendedor),
        )
        .route(
            "/api/v2/configuracion/sitio",
            get(configuracion_sitio::obtener).patch(configuracion_sitio::actualizar),
        )
        .route(
            "/api/v2/empresas/{empresa_id}/roles",
            get(personal::listar_roles).post(personal::crear_rol),
        )
        .route(
            "/api/v2/empresas/{empresa_id}/roles/{rol_id}",
            patch(personal::actualizar_rol),
        )
        .route(
            "/api/v2/empresas/{empresa_id}/puestos",
            get(personal::listar_puestos).post(personal::crear_puesto),
        )
        .route(
            "/api/v2/empresas/{empresa_id}/asignaciones-personal",
            get(personal::listar_asignaciones).post(personal::asignar),
        )
        .route(
            "/api/v2/empresas/{empresa_id}/invitaciones/personal",
            get(personal::listar_invitaciones).post(personal::crear_invitacion),
        )
        .route(
            "/api/v2/empresas/{empresa_id}/invitaciones/personal/{invitacion_id}/revocar",
            post(personal::revocar_invitacion),
        )
        .route(
            "/api/v2/incorporaciones/{token}",
            get(personal::consultar_invitacion),
        )
        .route(
            "/api/v2/incorporaciones/{token}/aceptar",
            post(personal::aceptar_invitacion),
        )
        .route(
            "/api/v2/empresas/{empresa_id}/capacidades",
            get(capacidades::listar),
        )
        .route(
            "/api/v2/empresas/{empresa_id}/capacidades/{capacidad}",
            patch(capacidades::actualizar),
        )
        .route("/api/v2/packs", get(packs::listar).post(packs::crear))
        .route(
            "/api/v2/packs/modelos-operacion",
            get(packs::listar_modelos).post(packs::crear_modelo),
        )
        .route(
            "/api/v2/packs/verticales",
            get(packs::listar_verticales).post(packs::crear_vertical),
        )
        .route("/api/v2/packs/{codigo}/{version}", get(packs::obtener))
        .fallback(no_encontrado)
        .with_state(state.clone());
    aplicar_capas(router, state)
}

/// Capas en el mismo orden que los interceptores globales de Nest:
/// correlación (más externa) → métricas → envelope → handler.
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

/// Cuerpo JSON con el shape de error de Nest cuando el payload es inválido.
/// Un cuerpo vacío se trata como `null` (igual que el body-parser de Express).
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
    let metrics = Metrics::nueva(config.servicio.clone());
    AppState::nueva(config, std::sync::Arc::new(metrics))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn config() -> Config {
        Config::from_pairs([("SERVICIO", "identity-service")]).expect("config")
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
        assert_eq!(valor["servicio"], "identity-service");
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
                HttpRequest::get("/api/v2/desconocido")
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
    async fn debe_envolver_las_rutas_publicas_y_propagar_request_id() {
        let state = estado_de_prueba(config());
        let app = aplicar_capas(
            Router::new()
                .route(
                    "/api/v2/auth/prueba",
                    get(|| async { Json(serde_json::json!({ "ok": true })) }),
                )
                .with_state(state.clone()),
            state,
        );
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/auth/prueba")
                    .header("x-request-id", "req-42")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.headers()["x-request-id"], "req-42");
        let valor: serde_json::Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["data"]["ok"], true);
        assert_eq!(valor["meta"]["path"], "/api/v2/auth/prueba");
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
        let valor: serde_json::Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["x-servicio"], "identity-service");
    }
}
