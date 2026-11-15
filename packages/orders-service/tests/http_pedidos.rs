//! Pruebas HTTP de autorización, validación y envelope (sin infraestructura).

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use orders_service::config::Config;
use orders_service::http::{build_router, estado_de_prueba};
use orders_service::metrics::Metrics;
use orders_service::state::AppState;
use serde_json::Value;
use tower::ServiceExt;

fn config() -> Config {
    Config::from_pairs([
        ("SERVICIO", "orders-service"),
        ("INTERNAL_API_KEY", "clave-interna"),
    ])
    .expect("config")
}

fn app() -> axum::Router {
    build_router(estado_de_prueba(config()))
}

async fn cuerpo(response: axum::response::Response) -> String {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("cuerpo")
        .to_bytes();
    String::from_utf8(bytes.to_vec()).expect("utf8")
}

fn comprador() -> [(&'static str, &'static str); 3] {
    [
        ("x-user-id", "33333333-3333-3333-3333-333333333333"),
        ("x-user-rol", "comprador"),
        ("x-user-email", "comprador@test.dev"),
    ]
}

#[tokio::test]
async fn debe_denegar_una_ruta_admin_a_un_comprador() {
    let mut request = Request::get("/api/v2/orders/admin/todas");
    for (clave, valor) in comprador() {
        request = request.header(clave, valor);
    }
    let response = app()
        .oneshot(request.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
    assert_eq!(valor["codigo"], "ACCESO_DENEGADO");
}

#[tokio::test]
async fn debe_validar_el_cuerpo_antes_de_tocar_dependencias() {
    let mut request = Request::post("/api/v2/orders").header("content-type", "application/json");
    for (clave, valor) in comprador() {
        request = request.header(clave, valor);
    }
    let response = app()
        .oneshot(
            request
                .body(Body::from(r#"{"tipo_orden":"otro"}"#))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
    assert_eq!(valor["codigo"], "VALIDACION");
    assert!(valor.get("meta").is_none());
}

#[tokio::test]
async fn debe_rechazar_el_carrito_sin_contexto() {
    let response = app()
        .oneshot(
            Request::get("/api/v2/carrito")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn debe_exigir_la_clave_interna_en_las_rutas_internas() {
    let response = app()
        .oneshot(
            Request::get("/internal/orders")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
    assert_eq!(valor["codigo"], "ACCESO_DENEGADO");
}

#[tokio::test]
async fn debe_validar_la_clave_interna_y_el_estado_requerido() {
    let response = app()
        .oneshot(
            Request::post("/internal/orders/123e4567-e89b-12d3-a456-426614174000/transicion")
                .header("x-internal-key", "clave-interna")
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
async fn debe_envolver_las_respuestas_publicas() {
    let state = AppState::nueva(config(), Metrics::nueva("orders-service"));
    let router = orders_service::http::aplicar_capas(
        axum::Router::new()
            .route(
                "/api/v2/prueba",
                axum::routing::get(|| async { axum::Json(serde_json::json!({"ok": true})) }),
            )
            .with_state(state.clone()),
        state,
    );
    let response = router
        .oneshot(
            Request::get("/api/v2/prueba")
                .header("x-request-id", "req-42")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.headers()["x-request-id"], "req-42");
    let valor: Value = serde_json::from_str(&cuerpo(response).await).expect("json");
    assert_eq!(valor["data"]["ok"], true);
    assert_eq!(valor["meta"]["path"], "/api/v2/prueba");
    assert_eq!(valor["meta"]["statusCode"], 200);
}
