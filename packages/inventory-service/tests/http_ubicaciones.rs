//! Pruebas HTTP de ubicaciones, movimientos y superficie interna.

mod common;

use axum::body::Body;
use axum::http::{Request as HttpRequest, StatusCode};
use common::*;
use http_body_util::BodyExt;
use inventory_service::auth::{CapacidadActiva, CapacidadesFijas};
use inventory_service::config::Config;
use inventory_service::http::build_router;
use inventory_service::metrics::Metrics;
use inventory_service::state::AppState;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

async fn estado() -> (tokio::sync::MutexGuard<'static, ()>, AppState, Arc<Db>) {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = db().await;
    let config = Config::from_pairs([
        ("SERVICIO", "inventory-service"),
        ("DATABASE_URL", url.as_str()),
        ("INTERNAL_API_KEY", "secreta"),
    ])
    .expect("config");
    let metrics = Arc::new(Metrics::nueva("inventory-service"));
    let mut state = AppState::con_db(
        config,
        metrics,
        Arc::clone(&db),
        Arc::new(CapacidadesFijas(vec![])),
    );
    state.capacidades = Arc::new(CapacidadesFijas(vec![
        CapacidadActiva {
            empresa_id: EMPRESA.to_string(),
            capacidad_codigo: "inventario_stock".to_string(),
            activo: true,
        },
        CapacidadActiva {
            empresa_id: EMPRESA.to_string(),
            capacidad_codigo: "inventario_lotes".to_string(),
            activo: true,
        },
    ]));
    (guard, state, db)
}

fn request(metodo: &str, ruta: &str) -> axum::http::request::Builder {
    HttpRequest::builder()
        .method(metodo)
        .uri(ruta)
        .header("x-user-id", "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
        .header("x-user-rol", "empresario")
        .header("x-tenant", EMPRESA)
        .header("content-type", "application/json")
}

async fn json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("cuerpo")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json")
}

#[tokio::test]
async fn debe_cubrir_ubicaciones_movimientos_y_superficie_interna() {
    let (_guard, state, db) = estado().await;
    crear_item(&db, ITEM, "SKU-HTTP", 10, 1000).await;
    let app = build_router(state);

    let creada = app
        .clone()
        .oneshot(
            request("POST", "/api/v2/admin/ubicaciones")
                .body(Body::from(
                    json!({"codigo": "central", "nombre": "Bodega Central"}).to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(creada.status(), StatusCode::CREATED);
    let central = json(creada).await;
    let central_id = central["data"]["id"].as_str().expect("id").to_string();

    let listado = app
        .clone()
        .oneshot(
            request("GET", "/api/v2/admin/ubicaciones")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(listado.status(), StatusCode::OK);
    let listado = json(listado).await;
    assert_eq!(listado["data"][0]["codigo"], "central");

    let stock_item = app
        .clone()
        .oneshot(
            request(
                "GET",
                &format!("/api/v2/admin/ubicaciones/items/{ITEM}/stock"),
            )
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(stock_item.status(), StatusCode::OK);
    assert_eq!(json(stock_item).await["data"]["stock_total"], 10);

    let segunda = app
        .clone()
        .oneshot(
            request("POST", "/api/v2/admin/ubicaciones")
                .body(Body::from(
                    json!({"codigo": "norte", "nombre": "Sucursal Norte"}).to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    let norte_id = json(segunda).await["data"]["id"]
        .as_str()
        .expect("id")
        .to_string();

    let traslado = app
        .clone()
        .oneshot(
            request("POST", "/api/v2/admin/ubicaciones/transferencias")
                .body(Body::from(
                    json!({
                        "item_id": ITEM,
                        "origen_id": central_id,
                        "destino_id": norte_id,
                        "cantidad": 3
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(traslado.status(), StatusCode::CREATED);
    assert_eq!(json(traslado).await["data"]["origen"]["stock"], 7);

    let fijado = app
        .clone()
        .oneshot(
            request(
                "PUT",
                &format!("/api/v2/admin/ubicaciones/{central_id}/stock/{ITEM}"),
            )
            .body(Body::from(json!({"stock": 12}).to_string()))
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(fijado.status(), StatusCode::OK);
    assert_eq!(json(fijado).await["data"]["stock"], 12);

    let actualizada = app
        .clone()
        .oneshot(
            request("PATCH", &format!("/api/v2/admin/ubicaciones/{central_id}"))
                .body(Body::from(
                    json!({"nombre": "Bodega Principal"}).to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(actualizada.status(), StatusCode::OK);

    let stock_ubicacion = app
        .clone()
        .oneshot(
            request(
                "GET",
                &format!("/api/v2/admin/ubicaciones/{central_id}/stock"),
            )
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(stock_ubicacion.status(), StatusCode::OK);

    let movimiento = app
        .clone()
        .oneshot(
            request("POST", "/api/v2/catalog/inventario/movimientos")
                .body(Body::from(
                    json!({
                        "tipo": "entrada",
                        "motivo": "compra",
                        "lineas": [{"sku": "SKU-HTTP", "cantidad": 2}]
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(movimiento.status(), StatusCode::CREATED);

    let movimientos = app
        .clone()
        .oneshot(
            request("GET", "/api/v2/catalog/inventario/movimientos")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(movimientos.status(), StatusCode::OK);
    assert_eq!(json(movimientos).await["data"]["total"], 1);

    let resumen = app
        .clone()
        .oneshot(
            request("GET", "/internal/inventario/resumen")
                .header("x-internal-key", "secreta")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(resumen.status(), StatusCode::OK);
    assert_eq!(json(resumen).await["productos"], 1);

    let lineas = app
        .clone()
        .oneshot(
            request("GET", "/internal/inventario/lineas")
                .header("x-internal-key", "secreta")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(lineas.status(), StatusCode::OK);

    let sin_clave = app
        .oneshot(
            request("GET", "/internal/inventario/resumen")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(sin_clave.status(), StatusCode::FORBIDDEN);
}
