//! Pruebas HTTP de infraestructura y guardas del router.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use catalog_service::config::Config;
use catalog_service::http::{build_router, estado_de_prueba};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

fn app() -> axum::Router {
    build_router(estado_de_prueba(
        Config::from_pairs([
            ("SERVICIO", "catalog-service"),
            ("INTERNAL_API_KEY", "secreta"),
        ])
        .expect("config"),
    ))
}

async fn enviar(
    metodo: &str,
    ruta: &str,
    cabeceras: &[(&str, &str)],
    cuerpo: Option<Value>,
) -> (StatusCode, Value) {
    let mut constructor = Request::builder().method(metodo).uri(ruta);
    for (clave, valor) in cabeceras {
        constructor = constructor.header(*clave, *valor);
    }
    let cuerpo = match cuerpo {
        Some(valor) => Body::from(valor.to_string()),
        None => Body::empty(),
    };
    let respuesta = app()
        .clone()
        .oneshot(constructor.body(cuerpo).expect("request"))
        .await
        .expect("response");
    let status = respuesta.status();
    let bytes = respuesta
        .into_body()
        .collect()
        .await
        .expect("cuerpo")
        .to_bytes();
    let valor = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, valor)
}

#[tokio::test]
async fn debe_exigir_autenticacion_validacion_y_clave_interna() {
    let (status, valor) = enviar("GET", "/api/v2/catalog/items", &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(valor["codigo"], "NO_AUTORIZADO");

    let admin = [
        ("x-user-id", common::USUARIO),
        ("x-user-rol", "admin"),
        ("x-tenant", common::EMPRESA),
    ];
    let (status, valor) = enviar(
        "POST",
        "/api/v2/catalog/items",
        &admin,
        Some(json!({"tipo": "producto"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(valor["codigo"], "VALIDACION");
    assert_eq!(valor["detalles"][0]["campo"], "codigo");

    let (status, valor) = enviar("GET", "/internal/items/lote", &[], None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(valor["codigo"], "ACCESO_DENEGADO");

    let (status, valor) = enviar("GET", "/api/v2/catalog/estado-inventario", &admin, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(valor["codigo"], "NO_ENCONTRADO");
}

#[tokio::test]
async fn debe_servir_salud_metricas_y_openapi_sin_envelope() {
    let (status, valor) = enviar("GET", "/health", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["servicio"], "catalog-service");
    assert!(valor.get("data").is_none());

    let (status, valor) = enviar("GET", "/docs-json", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["x-servicio"], "catalog-service");
}
