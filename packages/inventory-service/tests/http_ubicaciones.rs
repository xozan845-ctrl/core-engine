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
use tower::ServiceExt;

async fn estado() -> (DbLock, AppState, Arc<Db>) {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = provisionar(&url).await;
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
        None,
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

fn request_admin(metodo: &str, ruta: &str) -> axum::http::request::Builder {
    HttpRequest::builder()
        .method(metodo)
        .uri(ruta)
        .header("x-user-id", "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
        .header("x-user-rol", "admin")
        .header("x-tenant", EMPRESA)
        .header("content-type", "application/json")
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
    assert!(central["data"]["id"].is_string());

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
    let ubicaciones = listado["data"].as_array().expect("lista");
    assert!(ubicaciones
        .iter()
        .any(|ubicacion| ubicacion["codigo"] == "principal"));
    assert!(ubicaciones
        .iter()
        .any(|ubicacion| ubicacion["codigo"] == "central"));

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
    assert_eq!(segunda.status(), StatusCode::CREATED);

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

#[tokio::test]
async fn debe_proteger_y_validar_el_stock_inicial() {
    let (_guard, state, db) = estado().await;
    crear_item(&db, ITEM, "SKU-INICIAL-HTTP", 0, 1000).await;
    let app = build_router(state);

    let sin_clave = app
        .clone()
        .oneshot(
            request("POST", "/internal/inventario/stock/inicial")
                .body(Body::from(json!({}).to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(sin_clave.status(), StatusCode::FORBIDDEN);

    let invalido = app
        .clone()
        .oneshot(
            request("POST", "/internal/inventario/stock/inicial")
                .header("x-internal-key", "secreta")
                .body(Body::from(json!({"empresa_id": EMPRESA}).to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(invalido.status(), StatusCode::BAD_REQUEST);

    let valido = app
        .oneshot(
            request("POST", "/internal/inventario/stock/inicial")
                .header("x-internal-key", "secreta")
                .body(Body::from(
                    json!({
                        "empresa_id": EMPRESA,
                        "item_id": ITEM,
                        "sku": "SKU-INICIAL-HTTP",
                        "cantidad": 4
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(valido.status(), StatusCode::OK);
    assert_eq!(json(valido).await["stock"], 4);
}

#[tokio::test]
async fn debe_cubrir_validaciones_de_la_superficie_publica() {
    let (_guard, state, db) = estado().await;
    crear_item(&db, ITEM, "SKU-PUB", 5, 1000).await;
    let app = build_router(state);

    let inventario = app
        .clone()
        .oneshot(
            request_admin("GET", "/api/v2/admin/inventario")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(inventario.status(), StatusCode::OK);

    let proveedor_ok = app
        .clone()
        .oneshot(
            request_admin("POST", "/api/v2/admin/inventario/proveedores")
                .body(Body::from(
                    json!({"empresa_id": EMPRESA, "codigo": "PROV1", "nombre": "Proveedor"})
                        .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(proveedor_ok.status(), StatusCode::CREATED);

    let proveedor = app
        .clone()
        .oneshot(
            request_admin("POST", "/api/v2/admin/inventario/proveedores")
                .body(Body::from(json!({"nombre": "x"}).to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(proveedor.status(), StatusCode::BAD_REQUEST);

    let lotes = app
        .clone()
        .oneshot(
            request_admin(
                "GET",
                &format!("/api/v2/admin/inventario/lotes?empresa_id={EMPRESA}"),
            )
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(lotes.status(), StatusCode::OK);

    let lote_ok = app
        .clone()
        .oneshot(
            request_admin("POST", "/api/v2/admin/inventario/lotes")
                .body(Body::from(
                    json!({
                        "empresa_id": EMPRESA,
                        "item_id": ITEM,
                        "codigo_lote": "L-HTTP",
                        "cantidad_inicial": 5
                    })
                    .to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(lote_ok.status(), StatusCode::CREATED);

    let lote_invalido = app
        .clone()
        .oneshot(
            request_admin("POST", "/api/v2/admin/inventario/lotes")
                .body(Body::from(json!({"item_id": ITEM}).to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(lote_invalido.status(), StatusCode::BAD_REQUEST);

    let consumo_ok = app
        .clone()
        .oneshot(
            request_admin("POST", "/api/v2/admin/inventario/lotes/consumir")
                .body(Body::from(
                    json!({"empresa_id": EMPRESA, "item_id": ITEM, "cantidad": 1}).to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(consumo_ok.status(), StatusCode::CREATED);

    let consumo_invalido = app
        .clone()
        .oneshot(
            request_admin("POST", "/api/v2/admin/inventario/lotes/consumir")
                .body(Body::from(json!({"item_id": ITEM}).to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(consumo_invalido.status(), StatusCode::BAD_REQUEST);

    let configuracion = app
        .clone()
        .oneshot(
            request_admin(
                "GET",
                &format!("/api/v2/catalog/configuracion-inventario?empresa_id={EMPRESA}"),
            )
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(configuracion.status(), StatusCode::OK);

    let configuracion_invalida = app
        .clone()
        .oneshot(
            request_admin(
                "PUT",
                &format!("/api/v2/catalog/configuracion-inventario?empresa_id={EMPRESA}"),
            )
            .body(Body::from(json!({"metodo_costeo": "otro"}).to_string()))
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(configuracion_invalida.status(), StatusCode::BAD_REQUEST);

    let ubicacion_invalida = app
        .clone()
        .oneshot(
            request_admin("POST", "/api/v2/admin/ubicaciones")
                .body(Body::from(json!({"codigo": "X"}).to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(ubicacion_invalida.status(), StatusCode::BAD_REQUEST);

    let transferencia_invalida = app
        .clone()
        .oneshot(
            request_admin("POST", "/api/v2/admin/ubicaciones/transferencias")
                .body(Body::from(json!({}).to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(transferencia_invalida.status(), StatusCode::BAD_REQUEST);

    let listado = app
        .clone()
        .oneshot(
            request_admin(
                "GET",
                &format!("/api/v2/admin/ubicaciones?empresa_id={EMPRESA}"),
            )
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(listado.status(), StatusCode::OK);
    let ubicacion_id = json(listado).await["data"][0]["id"]
        .as_str()
        .expect("ubicacion")
        .to_string();

    let stock_item = app
        .clone()
        .oneshot(
            request_admin(
                "GET",
                &format!("/api/v2/admin/ubicaciones/items/{ITEM}/stock?empresa_id={EMPRESA}"),
            )
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(stock_item.status(), StatusCode::OK);

    let stock_ubicacion = app
        .clone()
        .oneshot(
            request_admin(
                "GET",
                &format!("/api/v2/admin/ubicaciones/{ubicacion_id}/stock?empresa_id={EMPRESA}"),
            )
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(stock_ubicacion.status(), StatusCode::OK);

    let stock_lote = app
        .clone()
        .oneshot(
            request("POST", "/internal/inventario/stock")
                .header("x-internal-key", "secreta")
                .body(Body::from(
                    json!({"empresa_id": EMPRESA, "skus": ["SKU-PUB"]}).to_string(),
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(stock_lote.status(), StatusCode::OK);

    let stock_lote_invalido = app
        .clone()
        .oneshot(
            request("POST", "/internal/inventario/stock")
                .header("x-internal-key", "secreta")
                .body(Body::from(json!({"empresa_id": EMPRESA}).to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(stock_lote_invalido.status(), StatusCode::BAD_REQUEST);

    let stock_sin_clave = app
        .clone()
        .oneshot(
            request("POST", "/internal/inventario/stock")
                .body(Body::from(json!({}).to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(stock_sin_clave.status(), StatusCode::FORBIDDEN);

    let no_encontrada = app
        .clone()
        .oneshot(
            request_admin(
                "GET",
                &format!("/api/v2/admin/ubicaciones/00000000-0000-4000-8000-000000000000/stock?empresa_id={EMPRESA}"),
            )
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(no_encontrada.status(), StatusCode::NOT_FOUND);
}
