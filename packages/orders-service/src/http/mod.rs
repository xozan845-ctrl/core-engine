use crate::auth::{exigir_clave_interna, UsuarioContexto};
use crate::errors::ApiError;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::state::AppState;
use axum::extract::{MatchedPath, Request, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;
use serde_json::Value;
use std::time::Instant;
use uuid::Uuid;

pub mod carrito;
pub mod contratos;
pub mod entregas;
pub mod interno;
pub mod pedidos;
pub mod salon;
pub mod suscripciones;
pub mod validacion;

/// Documento OpenAPI público versionado (golden del contrato, R-C-7).
const OPENAPI_PUBLICO: &str =
    include_str!("../../../../docs/openapi/orders-service/public.openapi.json");

const DOCS_HTML: &str = r#"<!doctype html>
<html lang="es">
  <head>
    <meta charset="utf-8" />
    <title>Core Engine · orders-service</title>
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
        .merge(pedidos::rutas())
        .merge(carrito::rutas())
        .merge(entregas::rutas())
        .merge(contratos::rutas())
        .merge(suscripciones::rutas())
        .merge(salon::rutas())
        .merge(interno::rutas())
        .fallback(no_encontrado)
        .with_state(state.clone());
    aplicar_capas(router, state)
}

/// Capas en el mismo orden que los interceptores globales de Nest.
pub fn aplicar_capas(router: Router, state: AppState) -> Router {
    router
        .layer(axum::middleware::from_fn(envelope::envelope))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            metricas,
        ))
        .layer(axum::middleware::from_fn_with_state(state, correlacion))
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
pub struct Cuerpo(pub Value);

impl<S> axum::extract::FromRequest<S> for Cuerpo
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, _state: &S) -> Result<Self, Self::Rejection> {
        let bytes = axum::body::to_bytes(request.into_body(), 100 * 1024)
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

pub fn ok<T: Serialize>(valor: T) -> Result<Response, ApiError> {
    Ok(Json(valor).into_response())
}

pub fn created<T: Serialize>(valor: T) -> Result<Response, ApiError> {
    Ok((StatusCode::CREATED, Json(valor)).into_response())
}

pub fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

/// Contexto autenticado + RolesGuard.
pub fn autorizar(
    headers: &axum::http::HeaderMap,
    roles: &[&str],
    permisos: &[&str],
) -> Result<UsuarioContexto, ApiError> {
    let usuario = UsuarioContexto::desde_headers_o_error(headers)?;
    usuario.exigir_roles(roles, permisos)?;
    Ok(usuario)
}

pub fn usuario_opcional(headers: &axum::http::HeaderMap) -> Option<UsuarioContexto> {
    UsuarioContexto::desde_headers(headers)
}

/// Equivalente a `CapabilityGuard`. Falla cerrado si identity no responde.
pub async fn exigir_capacidades(
    state: &AppState,
    usuario: &UsuarioContexto,
    capacidades: &[&str],
) -> Result<(), ApiError> {
    if capacidades.is_empty() || usuario.es_admin_global() {
        return Ok(());
    }
    let tenant = usuario.tenant()?;
    let empresas = vec![tenant.to_string()];
    let requeridas: Vec<String> = capacidades.iter().map(|c| c.to_string()).collect();
    let filas = state
        .clientes()?
        .capacidades(&empresas, &requeridas)
        .await
        .ok_or_else(|| ApiError::servicio_no_disponible("identity-service"))?;
    let activas: std::collections::HashSet<&str> = filas
        .iter()
        .filter(|fila| fila.activo && fila.empresa_id == tenant)
        .map(|fila| fila.capacidad_codigo.as_str())
        .collect();
    let faltantes: Vec<&str> = capacidades
        .iter()
        .copied()
        .filter(|capacidad| !activas.contains(capacidad))
        .collect();
    if !faltantes.is_empty() {
        return Err(ApiError::capacidad_no_activa(&faltantes, tenant));
    }
    Ok(())
}

pub fn exigir_interna(headers: &axum::http::HeaderMap, state: &AppState) -> Result<(), ApiError> {
    exigir_clave_interna(headers, state.config.internal_api_key.as_deref())
}

pub fn query<'a>(
    valores: &'a std::collections::HashMap<String, String>,
    clave: &str,
) -> Option<&'a str> {
    valores
        .get(clave)
        .map(String::as_str)
        .map(str::trim)
        .filter(|valor| !valor.is_empty())
}

pub fn query_numero(
    valores: &std::collections::HashMap<String, String>,
    clave: &str,
) -> Option<i64> {
    query(valores, clave).and_then(|valor| valor.parse().ok())
}

/// ID de correlación, logs de petición y contexto para RLS.
pub(crate) async fn correlacion(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|valor| valor.to_str().ok())
        .map(|valor| valor.trim().to_string())
        .filter(|valor| !valor.is_empty())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let headers = request.headers();
    let usuario = UsuarioContexto::desde_headers(headers);
    let interna = headers
        .get("x-internal-key")
        .and_then(|valor| valor.to_str().ok())
        .is_some_and(|clave| {
            !clave.is_empty()
                && state
                    .config
                    .internal_api_key
                    .as_deref()
                    .is_some_and(|esperada| clave == esperada)
        });
    let usuario_id = usuario
        .as_ref()
        .map(|u| u.user_id.clone())
        .or_else(|| interna.then(|| "00000000-0000-0000-0000-000000000000".to_string()));
    let rol = usuario
        .as_ref()
        .map(|u| u.rol.clone())
        .or_else(|| interna.then(|| "super_admin".to_string()));
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
        rol,
        tenant_id: usuario.as_ref().and_then(|u| u.tenant_id.clone()),
    };

    let mut response = ejecutar_con_contexto(contexto, async move {
        crate::logs::info(
            "http",
            crate::campos! {
                "msg": "peticion",
                "metodo": metodo,
                "ruta": ruta,
                "usuario": usuario.as_ref().map(|u| u.user_id.clone()),
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
pub(crate) async fn metricas(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
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
pub fn estado_de_prueba(config: crate::config::Config) -> AppState {
    AppState::nueva(
        config.clone(),
        crate::metrics::Metrics::nueva(config.servicio),
    )
}

mod envelope;
pub use envelope::aplicar_envelope;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request as HttpRequest;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn config() -> crate::config::Config {
        crate::config::Config::from_pairs([("SERVICIO", "orders-service")]).expect("config")
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
        assert_eq!(valor["servicio"], "orders-service");
        assert!(valor.get("data").is_none());
    }

    #[tokio::test]
    async fn debe_exponer_metricas_en_texto_prometheus() {
        let state = estado_de_prueba(config());
        let app = build_router(state);
        let response = app
            .oneshot(
                HttpRequest::get("/metrics")
                    .body(Body::empty())
                    .expect("request"),
            )
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
        let texto = cuerpo(response).await;
        assert!(texto.contains("core_engine_http_requests_total"));
    }

    #[tokio::test]
    async fn debe_responder_404_estructurado() {
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
        let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
        assert_eq!(valor["codigo"], "NO_ENCONTRADO");
    }

    #[tokio::test]
    async fn debe_exigir_contexto_en_rutas_privadas() {
        let app = build_router(estado_de_prueba(config()));
        let response = app
            .oneshot(
                HttpRequest::get("/api/v2/orders")
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
        assert_eq!(valor["x-servicio"], "orders-service");
    }
}
