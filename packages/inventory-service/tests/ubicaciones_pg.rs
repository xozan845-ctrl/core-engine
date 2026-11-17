//! Pruebas PostgreSQL de ubicaciones, transferencias y movimientos.

mod common;

use common::*;
use inventory_service::movimientos::Movimientos;
use inventory_service::outbox::Outbox;
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
    let outbox = Arc::new(Outbox::nueva("inventory.outbox"));
    let ubicaciones = Arc::new(Ubicaciones::nueva(
        Arc::clone(&db),
        Some(Arc::clone(&outbox)),
    ));
    let movimientos = Arc::new(Movimientos::nueva(Arc::clone(&db), Some(outbox)));
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
            None,
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
    let principal_id = detalle["ubicaciones"][0]["ubicacion_id"]
        .as_str()
        .expect("principal")
        .to_string();

    let sucursal = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            None,
            &json!({"codigo": "sucursal", "nombre": "Sucursal Norte", "tipo": "sucursal"}),
        )
        .await
        .expect("sucursal");
    let sucursal_id = sucursal["id"].as_str().expect("id").to_string();

    ctx.ubicaciones
        .transferir(
            EMPRESA,
            None,
            &json!({
                "item_id": ITEM,
                "origen_id": principal_id,
                "destino_id": central_id,
                "cantidad": 10
            }),
        )
        .await
        .expect("traslado inicial");

    let traslado = ctx
        .ubicaciones
        .transferir(
            EMPRESA,
            None,
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
            None,
            &json!({"codigo": "bodega-a", "nombre": "Bodega A"}),
        )
        .await
        .expect("a");
    let b = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            None,
            &json!({"codigo": "bodega-b", "nombre": "Bodega B"}),
        )
        .await
        .expect("b");
    let error = ctx
        .ubicaciones
        .transferir(
            EMPRESA,
            None,
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
    ctx.ubicaciones
        .crear(
            EMPRESA,
            None,
            &json!({"codigo": "principal", "nombre": "Principal", "tipo": "bodega"}),
        )
        .await
        .expect("ubicacion");
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

    let stock: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(stock), 0)::bigint
           FROM inventory.item_stock_ubicacion WHERE item_id = $1::uuid",
    )
    .bind(ITEM)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock");
    assert_eq!(stock.0, 15);

    let por_ubicacion: i32 = sqlx::query_scalar(
        "SELECT stock FROM inventory.item_stock_ubicacion
          WHERE item_id = $1::uuid",
    )
    .bind(ITEM)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock ubicacion");
    assert_eq!(por_ubicacion, 15);

    let eventos: Vec<(String, serde_json::Value)> =
        sqlx::query_as("SELECT tipo, payload FROM inventory.outbox ORDER BY creado_en")
            .fetch_all(ctx.db.pool())
            .await
            .expect("outbox");
    assert_eq!(eventos.len(), 1);
    assert_eq!(eventos[0].0, "stock.updated");
    assert_eq!(eventos[0].1["data"]["items"][0]["sku"], "SKU-MOV");
    assert_eq!(eventos[0].1["data"]["items"][0]["stock_restante"], 15);

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

#[tokio::test]
async fn debe_crear_stock_inicial_en_la_ubicacion_predeterminada() {
    let ctx = preparar().await;
    ctx.ubicaciones
        .crear(
            EMPRESA,
            None,
            &json!({"codigo": "principal", "nombre": "Principal", "tipo": "bodega"}),
        )
        .await
        .expect("ubicacion");

    crear_item(&ctx.db, ITEM, "SKU-INICIAL", 7, 1000).await;

    let ubicacion_stock: i32 = sqlx::query_scalar(
        "SELECT stock FROM inventory.item_stock_ubicacion WHERE item_id = $1::uuid",
    )
    .bind(ITEM)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock inicial");
    assert_eq!(ubicacion_stock, 7);
}

#[tokio::test]
async fn debe_fijar_stock_por_ubicacion_y_publicar_evento() {
    let ctx = preparar().await;
    let ubicacion = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            None,
            &json!({"codigo": "principal", "nombre": "Principal", "tipo": "bodega"}),
        )
        .await
        .expect("ubicacion");
    let ubicacion_id = ubicacion["id"].as_str().expect("id").to_string();
    crear_item(&ctx.db, ITEM, "SKU-AJUSTE", 0, 1000).await;

    ctx.ubicaciones
        .fijar_stock(EMPRESA, &ubicacion_id, ITEM, &json!(12))
        .await
        .expect("ajuste");

    let stock: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(stock), 0)::bigint
           FROM inventory.item_stock_ubicacion WHERE item_id = $1::uuid",
    )
    .bind(ITEM)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock");
    assert_eq!(stock.0, 12);

    let ubicacion_stock: i32 = sqlx::query_scalar(
        "SELECT stock FROM inventory.item_stock_ubicacion
          WHERE item_id = $1::uuid AND ubicacion_id = $2::uuid",
    )
    .bind(ITEM)
    .bind(&ubicacion_id)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock ubicacion");
    assert_eq!(ubicacion_stock, 12);

    let evento: (String, serde_json::Value) = sqlx::query_as(
        "SELECT tipo, payload FROM inventory.outbox ORDER BY creado_en DESC LIMIT 1",
    )
    .fetch_one(ctx.db.pool())
    .await
    .expect("evento");
    assert_eq!(evento.0, "stock.updated");
    assert_eq!(evento.1["data"]["tipo"], "ajuste_ubicacion");
    assert_eq!(evento.1["data"]["items"][0]["stock_restante"], 12);

    // Un lote vacío no debe publicar nada.
    let outbox = Outbox::nueva("inventory.outbox");
    let mut tx = ctx.db.transaccion().await.expect("tx");
    outbox
        .insertar_stock_updated(&mut tx, "sin-items", "ajuste", vec![])
        .await
        .expect("sin items");
    tx.commit().await.expect("commit");
    let eventos: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM inventory.outbox")
        .fetch_one(ctx.db.pool())
        .await
        .expect("eventos");
    assert_eq!(eventos, 1);
}

#[tokio::test]
async fn debe_filtrar_movimientos_por_negocio() {
    let ctx = preparar().await;
    let negocio = "22222222-2222-4222-8222-222222222222";
    ctx.ubicaciones
        .crear(
            EMPRESA,
            Some(negocio),
            &json!({"codigo": "principal", "nombre": "Principal", "tipo": "bodega"}),
        )
        .await
        .expect("ubicacion");
    crear_item(&ctx.db, ITEM, "SKU-MOV-NEG", 10, 1000).await;
    sqlx::query("UPDATE item.items SET negocio_id = $1::uuid WHERE id = $2::uuid")
        .bind(negocio)
        .bind(ITEM)
        .execute(ctx.db.pool())
        .await
        .expect("negocio");
    ctx.movimientos
        .crear(
            EMPRESA,
            &json!({"tipo": "entrada", "lineas": [{"sku": "SKU-MOV-NEG", "cantidad": 2}]}),
            None,
        )
        .await
        .expect("movimiento");

    let pagina = ctx
        .movimientos
        .listar(EMPRESA, &json!({"negocio_id": negocio}))
        .await
        .expect("listar negocio");
    assert_eq!(pagina["total"], 1);

    let otro = ctx
        .movimientos
        .listar(
            EMPRESA,
            &json!({"negocio_id": "33333333-3333-4333-8333-333333333333"}),
        )
        .await
        .expect("listar otro");
    assert_eq!(otro["total"], 0);
}

#[tokio::test]
async fn debe_actualizar_ubicaciones_respetando_el_negocio() {
    let ctx = preparar().await;
    let negocio = "22222222-2222-4222-8222-222222222222";
    let otra = "33333333-3333-4333-8333-333333333333";
    let ubicacion = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            Some(negocio),
            &json!({"codigo": "norte", "nombre": "Norte", "tipo": "bodega"}),
        )
        .await
        .expect("ubicacion");
    let id = ubicacion["id"].as_str().expect("id").to_string();
    crear_item(&ctx.db, ITEM, "SKU-UPD", 10, 1000).await;
    ctx.ubicaciones
        .fijar_stock(EMPRESA, &id, ITEM, &json!(5))
        .await
        .expect("stock");

    let error = ctx
        .ubicaciones
        .actualizar(EMPRESA, Some(otra), &id, &json!({"nombre": "Otra"}))
        .await
        .expect_err("otro negocio");
    assert_eq!(error.codigo, "NO_ENCONTRADO");

    let error = ctx
        .ubicaciones
        .actualizar(EMPRESA, Some(negocio), &id, &json!({"activo": false}))
        .await
        .expect_err("con stock");
    assert_eq!(error.codigo, "CONFLICTO");

    let actualizada = ctx
        .ubicaciones
        .actualizar(
            EMPRESA,
            Some(negocio),
            &id,
            &json!({"nombre": "Norte 2", "predeterminada": true}),
        )
        .await
        .expect("actualizar");
    assert_eq!(actualizada["nombre"], "Norte 2");
    assert_eq!(actualizada["predeterminada"], true);
}

#[tokio::test]
async fn debe_actualizar_ubicaciones_y_rechazar_cambios_invalidos() {
    let ctx = preparar().await;
    let negocio = "22222222-2222-4222-8222-222222222222";
    let otra = "33333333-3333-4333-8333-333333333333";
    let ubicacion = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            Some(negocio),
            &json!({"codigo": "norte", "nombre": "Norte", "tipo": "bodega"}),
        )
        .await
        .expect("ubicacion");
    let id = ubicacion["id"].as_str().expect("id").to_string();
    crear_item(&ctx.db, ITEM, "SKU-UPD", 5, 1000).await;
    ctx.ubicaciones
        .fijar_stock(EMPRESA, &id, ITEM, &json!(5))
        .await
        .expect("stock");

    let error = ctx
        .ubicaciones
        .actualizar(EMPRESA, Some(otra), &id, &json!({"nombre": "Otra"}))
        .await
        .expect_err("otro negocio");
    assert_eq!(error.codigo, "NO_ENCONTRADO");

    let error = ctx
        .ubicaciones
        .actualizar(EMPRESA, Some(negocio), &id, &json!({"activo": false}))
        .await
        .expect_err("con stock");
    assert_eq!(error.codigo, "CONFLICTO");

    let actualizada = ctx
        .ubicaciones
        .actualizar(
            EMPRESA,
            Some(negocio),
            &id,
            &json!({"nombre": "Norte 2", "predeterminada": true}),
        )
        .await
        .expect("actualizar");
    assert_eq!(actualizada["nombre"], "Norte 2");

    let listado = ctx
        .ubicaciones
        .listar(EMPRESA, Some(negocio))
        .await
        .expect("listar negocio");
    assert_eq!(listado.len(), 1);
    let todas = ctx
        .ubicaciones
        .listar(EMPRESA, None)
        .await
        .expect("listar todas");
    assert!(!todas.is_empty());
}

#[tokio::test]
async fn debe_rechazar_operaciones_invalidas_de_ubicaciones() {
    let ctx = preparar().await;
    let vacia = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            None,
            &json!({"codigo": "vacia", "nombre": "Vacía", "tipo": "bodega"}),
        )
        .await
        .expect("ubicacion");
    let vacia_id = vacia["id"].as_str().expect("id").to_string();
    crear_item(&ctx.db, ITEM, "SKU-INV", 5, 1000).await;

    let error = ctx
        .ubicaciones
        .transferir(
            EMPRESA,
            None,
            &json!({
                "item_id": ITEM,
                "origen_id": vacia_id,
                "destino_id": vacia_id,
                "cantidad": 1
            }),
        )
        .await
        .expect_err("mismo destino");
    assert_eq!(error.codigo, "TRASLADO_INVALIDO");

    let error = ctx
        .ubicaciones
        .stock_por_item(EMPRESA, "00000000-0000-4000-8000-000000000000")
        .await
        .expect_err("item inexistente");
    assert_eq!(error.codigo, "NO_ENCONTRADO");

    let error = ctx
        .ubicaciones
        .stock_de_ubicacion(EMPRESA, "00000000-0000-4000-8000-000000000000")
        .await
        .expect_err("ubicacion inexistente");
    assert_eq!(error.codigo, "NO_ENCONTRADO");

    sqlx::query(
        "INSERT INTO item.items (id, empresa_id, tipo, codigo, nombre, track_inventory, estado)
         VALUES ('33333333-3333-4333-8333-333333333333', $1::uuid, 'producto', 'SIN-INV', 'Sin inventario', false, 'activo')",
    )
    .bind(EMPRESA)
    .execute(ctx.db.pool())
    .await
    .expect("item sin inventario");
    let error = ctx
        .ubicaciones
        .fijar_stock(
            EMPRESA,
            &vacia_id,
            "33333333-3333-4333-8333-333333333333",
            &json!(1),
        )
        .await
        .expect_err("sin inventario");
    assert_eq!(error.codigo, "INVENTARIO_NO_APLICA");
}
