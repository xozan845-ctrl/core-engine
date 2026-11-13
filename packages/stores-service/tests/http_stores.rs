//! Pruebas HTTP completas del router (envelope, guardas y rutas internas).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use stores_service::config::Config;
use stores_service::http::{build_router, estado_de_prueba};
use stores_service::metrics::Metrics;
use stores_service::ofertas::Ofertas;
use stores_service::state::AppState;
use stores_service::tiendas::Tiendas;
use tokio::sync::Mutex;
use tower::ServiceExt;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

fn estado_completo(db: Arc<stores_service::db::Db>) -> AppState {
    let config = Config::from_pairs([
        ("SERVICIO", "stores-service"),
        ("INTERNAL_API_KEY", "secreta"),
    ])
    .expect("config");
    let temas = temas(None, json!({}), None, json!({}));
    let storage = storage_vacio();
    let catalogo = catalogo("1000.00", Some(5), true);
    let tiendas = Arc::new(Tiendas::nueva(
        Arc::clone(&db),
        Arc::clone(&temas),
        Arc::clone(&storage),
    ));
    let ofertas = Arc::new(Ofertas::nueva(Arc::clone(&db), Arc::clone(&catalogo)));
    AppState::con_dependencias(
        config,
        Arc::new(Metrics::nueva("stores-service")),
        capacidades_ok(),
        temas,
        storage,
        catalogo,
        tiendas,
        ofertas,
    )
}

async fn enviar(
    app: &axum::Router,
    metodo: &str,
    ruta: &str,
    cabeceras: &[(&str, &str)],
    cuerpo: Option<Value>,
) -> (StatusCode, Value, Option<String>) {
    let mut constructor = Request::builder().method(metodo).uri(ruta);
    for (clave, valor) in cabeceras {
        constructor = constructor.header(*clave, *valor);
    }
    let cuerpo = match cuerpo {
        Some(valor) => Body::from(valor.to_string()),
        None => Body::empty(),
    };
    let respuesta = app
        .clone()
        .oneshot(constructor.body(cuerpo).expect("request"))
        .await
        .expect("response");
    let status = respuesta.status();
    let location = respuesta
        .headers()
        .get("location")
        .and_then(|valor| valor.to_str().ok())
        .map(str::to_string);
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
    (status, valor, location)
}

#[tokio::test]
async fn debe_exigir_contexto_validacion_y_clave_interna() {
    let app = build_router(estado_de_prueba(
        Config::from_pairs([
            ("SERVICIO", "stores-service"),
            ("INTERNAL_API_KEY", "secreta"),
        ])
        .expect("config"),
    ));

    let (status, valor, _) = enviar(&app, "GET", "/api/v2/vendedores/me/tienda", &[], None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(valor["codigo"], "NO_AUTORIZADO");

    let (status, valor, _) = enviar(
        &app,
        "POST",
        "/api/v2/vendedores/tienda",
        &[
            ("x-user-id", USUARIO),
            ("x-user-rol", "empresario"),
            ("x-tenant", EMPRESA),
        ],
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(valor["codigo"], "VALIDACION");
    assert_eq!(valor["detalles"][0]["campo"], "nombre");

    let (status, valor, _) = enviar(&app, "GET", "/internal/tiendas", &[], None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(valor["codigo"], "ACCESO_DENEGADO");

    let (status, valor, _) = enviar(&app, "GET", "/api/v2/tiendas/desconocida", &[], None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(valor["codigo"], "ERROR_INTERNO");
}

#[tokio::test]
async fn debe_recorrer_el_flujo_de_tienda_y_oferta_por_http() {
    let url = url().expect("TEST_DATABASE_URL");
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let app = build_router(estado_completo(db().await));
    let vendedor = [
        ("x-user-id", USUARIO),
        ("x-user-rol", "vendedor"),
        ("x-tenant", EMPRESA),
        ("x-user-participacion", "10"),
    ];

    let (status, valor, _) = enviar(
        &app,
        "POST",
        "/api/v2/vendedores/tienda",
        &vendedor,
        Some(json!({"nombre": "Mi Tienda", "slug": "mi-tienda"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(valor["data"]["slug"], "mi-tienda");
    assert_eq!(valor["meta"]["statusCode"], 201);
    let tienda_id = valor["data"]["id"].as_str().expect("id").to_string();

    let (status, valor, _) =
        enviar(&app, "GET", "/api/v2/vendedores/me/tienda", &vendedor, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["data"]["id"], tienda_id);

    let (status, valor, _) = enviar(
        &app,
        "POST",
        "/api/v2/vendedores/productos",
        &vendedor,
        Some(json!({
            "tienda_id": tienda_id,
            "item_id": ITEM,
            "margen": 15,
            "moneda": "nio"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(valor["data"]["precio_base"], "1000.00");
    assert_eq!(valor["data"]["precio_venta"], "1150.00");
    let oferta_id = valor["data"]["id"].as_str().expect("oferta").to_string();

    let (status, valor, _) = enviar(
        &app,
        "PATCH",
        &format!("/api/v2/vendedores/productos/{oferta_id}"),
        &vendedor,
        Some(json!({"margen": 20})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["data"]["precio_venta"], "1200.00");

    let (status, valor, _) = enviar(
        &app,
        "PATCH",
        &format!("/api/v2/tiendas/{tienda_id}/slug"),
        &vendedor,
        Some(json!({"slug": "mi-tienda-nueva"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["data"]["slug"], "mi-tienda-nueva");

    let (status, valor, location) =
        enviar(&app, "GET", "/api/v2/tiendas/mi-tienda", &[], None).await;
    assert_eq!(status, StatusCode::MOVED_PERMANENTLY);
    assert_eq!(location.as_deref(), Some("/api/v2/tiendas/mi-tienda-nueva"));
    assert!(valor.get("meta").is_none());
    assert_eq!(valor["tienda"]["slug"], "mi-tienda-nueva");

    let (status, valor, _) =
        enviar(&app, "GET", "/api/v2/tiendas/mi-tienda-nueva", &[], None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["data"]["tienda"]["slug"], "mi-tienda-nueva");
    assert_eq!(
        valor["data"]["ofertas"].as_array().expect("ofertas").len(),
        1
    );
    assert!(valor["data"]["ofertas"][0].get("margen").is_none());

    let (status, valor, _) = enviar(
        &app,
        "GET",
        &format!("/internal/tiendas/{tienda_id}"),
        &[("x-internal-key", "secreta")],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["id"], tienda_id);
    assert!(valor.get("meta").is_none());

    let (status, valor, _) = enviar(
        &app,
        "POST",
        "/api/v2/vendedores/tienda",
        &[
            ("x-user-id", USUARIO),
            ("x-user-rol", "empresario"),
            ("x-tenant", EMPRESA),
        ],
        Some(json!({"nombre": "Tienda Libre", "slug": "libre"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let libre_id = valor["data"]["id"].as_str().expect("libre").to_string();

    let (status, valor, _) = enviar(
        &app,
        "POST",
        &format!("/internal/tiendas/{libre_id}/asignar-vendedor"),
        &[("x-internal-key", "secreta")],
        Some(json!({"usuario_id": OTRO_USUARIO, "participacion": 50})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(valor["ok"], true);
}
