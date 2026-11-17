//! Pruebas HTTP de infraestructura y guardas del router.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use item_service::config::Config;
use item_service::http::{build_router, estado_de_prueba};
use serde_json::{json, Value};
use tower::ServiceExt;

fn app() -> axum::Router {
    build_router(estado_de_prueba(
        Config::from_pairs([
            ("SERVICIO", "item-service"),
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

    let (status, valor) = enviar("GET", "/internal/v2/items/lote", &[], None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(valor["codigo"], "ACCESO_DENEGADO");

    // El contrato interno v1 se retiró (ADR-0020 fase 4).
    let (status, _) = enviar("GET", "/internal/items/lote", &[], None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, valor) = enviar("GET", "/api/v2/catalog/estado-inventario", &admin, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(valor["codigo"], "NO_ENCONTRADO");

    // Las rutas PIM de series y componentes deben existir (no 404). Sin base
    // de datos fallan con 500, que es justo lo que distingue "registrada" de
    // "ruta retirada" en este estado de prueba.
    for ruta in [
        "/api/v2/catalog/items/00000000-0000-4000-8000-000000000000/series",
        "/api/v2/catalog/items/00000000-0000-4000-8000-000000000000/componentes",
    ] {
        let (status, _) = enviar("GET", ruta, &admin, None).await;
        assert_ne!(status, StatusCode::NOT_FOUND, "ruta ausente: {ruta}");
    }

    // Contrato v2: las rutas internas deben existir (sin DB fallan con 500).
    for ruta in [
        "/internal/v2/items/lote?skus=SKU-1",
        "/internal/v2/items/00000000-0000-4000-8000-000000000000",
    ] {
        let (status, _) = enviar("GET", ruta, &[("x-internal-key", "secreta")], None).await;
        assert_ne!(status, StatusCode::NOT_FOUND, "ruta ausente: {ruta}");
    }
}

#[tokio::test]
async fn debe_servir_salud_metricas_y_openapi_sin_envelope() {
    let (status, valor) = enviar("GET", "/health", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["servicio"], "item-service");
    assert!(valor.get("data").is_none());

    let (status, valor) = enviar("GET", "/docs-json", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["x-servicio"], "item-service");
}
