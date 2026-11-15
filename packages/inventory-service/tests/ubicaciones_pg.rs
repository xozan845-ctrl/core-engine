//! Pruebas PostgreSQL de ubicaciones, transferencias y movimientos.

mod common;

use common::*;
use inventory_service::movimientos::Movimientos;
use inventory_service::ubicaciones::Ubicaciones;
use serde_json::json;
use std::sync::Arc;

struct Contexto {
    _guard: DbLock,
    ubicaciones: Arc<Ubicaciones>,
    movimientos: Arc<Movimientos>,
    db: Arc<Db>,
}

async fn preparar() -> Contexto {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = provisionar(&url).await;
    let db = db().await;
    let ubicaciones = Arc::new(Ubicaciones::nueva(Arc::clone(&db)));
    let movimientos = Arc::new(Movimientos::nueva(Arc::clone(&db)));
    Contexto {
        _guard: guard,
        ubicaciones,
        movimientos,
        db,
    }
}

#[tokio::test]
async fn debe_absorber_stock_en_la_primera_ubicacion_y_transferir() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-UBI", 10, 1000).await;

    let central = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            &json!({"codigo": "central", "nombre": "Bodega Central", "tipo": "bodega"}),
        )
        .await
        .expect("central");
    let central_id = central["id"].as_str().expect("id").to_string();

    let detalle = ctx
        .ubicaciones
        .stock_por_item(EMPRESA, ITEM)
        .await
        .expect("stock item");
    assert_eq!(detalle["stock_total"], 10);
    assert_eq!(detalle["total_ubicaciones"], 10);
    assert_eq!(detalle["ubicaciones"][0]["stock"], 10);

    let sucursal = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            &json!({"codigo": "sucursal", "nombre": "Sucursal Norte", "tipo": "sucursal"}),
        )
        .await
        .expect("sucursal");
    let sucursal_id = sucursal["id"].as_str().expect("id").to_string();

    let traslado = ctx
        .ubicaciones
        .transferir(
            EMPRESA,
            &json!({
                "item_id": ITEM,
                "origen_id": central_id,
                "destino_id": sucursal_id,
                "cantidad": 4
            }),
        )
        .await
        .expect("traslado");
    assert_eq!(traslado["origen"]["stock"], 6);
    assert_eq!(traslado["destino"]["stock"], 4);

    let fijado = ctx
        .ubicaciones
        .fijar_stock(EMPRESA, &central_id, ITEM, &json!(20))
        .await
        .expect("fijar");
    assert_eq!(fijado["stock"], 20);
    assert_eq!(fijado["stock_total"], 24);

    let por_ubicacion = ctx
        .ubicaciones
        .stock_de_ubicacion(EMPRESA, &central_id)
        .await
        .expect("stock ubicacion");
    assert_eq!(por_ubicacion.len(), 1);
    assert_eq!(por_ubicacion[0]["sku"], "SKU-UBI");
}

#[tokio::test]
async fn debe_rechazar_un_traslado_sin_stock_suficiente() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-UBI-2", 2, 1000).await;
    let a = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            &json!({"codigo": "bodega-a", "nombre": "Bodega A"}),
        )
        .await
        .expect("a");
    let b = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            &json!({"codigo": "bodega-b", "nombre": "Bodega B"}),
        )
        .await
        .expect("b");
    let error = ctx
        .ubicaciones
        .transferir(
            EMPRESA,
            &json!({
                "item_id": ITEM,
                "origen_id": a["id"],
                "destino_id": b["id"],
                "cantidad": 3
            }),
        )
        .await
        .expect_err("stock");
    assert_eq!(error.codigo, "STOCK_UBICACION_INSUFICIENTE");
}

#[tokio::test]
async fn debe_registrar_un_movimiento_de_entrada_y_listarlo() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-MOV", 10, 1000).await;
    let movimiento = ctx
        .movimientos
        .crear(
            EMPRESA,
            &json!({
                "tipo": "entrada",
                "motivo": "compra",
                "referencia": "FAC-1",
                "lineas": [{"sku": "SKU-MOV", "cantidad": 5}]
            }),
            Some("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"),
        )
        .await
        .expect("movimiento");
    assert_eq!(movimiento["tipo"], "entrada");
    assert_eq!(movimiento["motivo"], "compra");
    assert_eq!(movimiento["lineas"][0]["stock_anterior"], 10);
    assert_eq!(movimiento["lineas"][0]["stock_nuevo"], 15);

    let stock: (i32,) = sqlx::query_as("SELECT stock FROM catalog.items WHERE id = $1::uuid")
        .bind(ITEM)
        .fetch_one(ctx.db.pool())
        .await
        .expect("stock");
    assert_eq!(stock.0, 15);

    let pagina = ctx
        .movimientos
        .listar(EMPRESA, &json!({"pagina": 1, "limite": 20}))
        .await
        .expect("listar");
    assert_eq!(pagina["total"], 1);
    assert_eq!(pagina["items"][0]["tipo"], "entrada");
}

#[tokio::test]
async fn debe_rechazar_una_salida_sin_stock_suficiente() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-MOV-2", 1, 1000).await;
    let error = ctx
        .movimientos
        .crear(
            EMPRESA,
            &json!({
                "tipo": "salida",
                "lineas": [{"sku": "SKU-MOV-2", "cantidad": 5}]
            }),
            None,
        )
        .await
        .expect_err("stock");
    assert_eq!(error.codigo, "STOCK_INSUFICIENTE");
}
