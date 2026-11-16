//! Pruebas PostgreSQL de la saga: reserva, rechazo, devolución e idempotencia.

mod common;

use common::*;
use inventory_service::consumidor::PedidosConsumer;
use inventory_service::outbox::Outbox;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

struct Contexto {
    _guard: DbLock,
    consumidor: Arc<PedidosConsumer>,
    db: Arc<Db>,
}

async fn preparar() -> Contexto {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = provisionar(&url).await;
    let db = db().await;
    let outbox = Arc::new(Outbox::nueva("inventory.outbox"));
    let consumidor = Arc::new(PedidosConsumer::nuevo(Arc::clone(&db), outbox));
    Contexto {
        _guard: guard,
        consumidor,
        db,
    }
}

fn evento(tipo: &str, event_id: Uuid, data: Value) -> Value {
    json!({
        "event_id": event_id.to_string(),
        "tipo": tipo,
        "data": data
    })
}

async fn outbox_tipos(db: &Db) -> Vec<String> {
    sqlx::query_scalar("SELECT tipo FROM inventory.outbox ORDER BY creado_en")
        .fetch_all(db.pool())
        .await
        .expect("outbox")
}

async fn stock(db: &Db) -> i32 {
    sqlx::query_scalar("SELECT stock FROM item.items WHERE id = $1::uuid")
        .bind(ITEM)
        .fetch_one(db.pool())
        .await
        .expect("stock")
}

#[tokio::test]
async fn debe_reservar_stock_y_emitir_eventos_en_una_transaccion() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-SAGA", 10, 1000).await;
    let order_id = Uuid::new_v4();
    let event_id = Uuid::new_v4();
    ctx.consumidor
        .manejar(evento(
            "order.created",
            event_id,
            json!({
                "order_id": order_id,
                "items": [{"empresa_id": EMPRESA, "sku": "SKU-SAGA", "cantidad": 3}]
            }),
        ))
        .await
        .expect("reserva");

    assert_eq!(stock(&ctx.db).await, 7);
    assert_eq!(
        outbox_tipos(&ctx.db).await,
        vec!["stock.reservado", "stock.updated"]
    );
    let reservas: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory.reservas_ordenes WHERE order_id = $1")
            .bind(order_id)
            .fetch_one(ctx.db.pool())
            .await
            .expect("reservas");
    assert_eq!(reservas, 1);
}

#[tokio::test]
async fn debe_ser_idempotente_por_event_id() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-IDEMP", 10, 1000).await;
    let evento = evento(
        "order.created",
        Uuid::new_v4(),
        json!({
            "order_id": Uuid::new_v4(),
            "items": [{"empresa_id": EMPRESA, "sku": "SKU-IDEMP", "cantidad": 4}]
        }),
    );
    ctx.consumidor
        .manejar(evento.clone())
        .await
        .expect("primera");
    ctx.consumidor
        .manejar(evento.clone())
        .await
        .expect("segunda");
    assert_eq!(stock(&ctx.db).await, 6);
    assert_eq!(outbox_tipos(&ctx.db).await.len(), 2);
}

#[tokio::test]
async fn debe_rechazar_todo_el_stock_si_una_linea_no_alcanza() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-FALLA", 2, 1000).await;
    let order_id = Uuid::new_v4();
    ctx.consumidor
        .manejar(evento(
            "order.created",
            Uuid::new_v4(),
            json!({
                "order_id": order_id,
                "items": [{"empresa_id": EMPRESA, "sku": "SKU-FALLA", "cantidad": 3}]
            }),
        ))
        .await
        .expect("fallo controlado");
    assert_eq!(stock(&ctx.db).await, 2);
    assert_eq!(outbox_tipos(&ctx.db).await, vec!["stock.fallido"]);
    let reservas: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory.reservas_ordenes WHERE order_id = $1")
            .bind(order_id)
            .fetch_one(ctx.db.pool())
            .await
            .expect("reservas");
    assert_eq!(reservas, 0);
}

#[tokio::test]
async fn debe_reintegrar_stock_y_series_en_la_devolucion() {
    let ctx = preparar().await;
    sqlx::query(
        "INSERT INTO item.items
           (id, empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, track_serial,
            stock, estado)
         VALUES ($1::uuid, $2::uuid, 'producto', 'SKU-SERIAL', 'Serial', 1000, true, true, 1, 'activo')",
    )
    .bind(ITEM)
    .bind(EMPRESA)
    .execute(ctx.db.pool())
    .await
    .expect("item");
    sqlx::query(
        "INSERT INTO item.item_series (empresa_id, item_id, serial, estado)
         VALUES ($1::uuid, $2::uuid, 'SER-1', 'disponible')",
    )
    .bind(EMPRESA)
    .bind(ITEM)
    .execute(ctx.db.pool())
    .await
    .expect("serie");

    let order_id = Uuid::new_v4();
    ctx.consumidor
        .manejar(evento(
            "order.created",
            Uuid::new_v4(),
            json!({
                "order_id": order_id,
                "items": [{"empresa_id": EMPRESA, "sku": "SKU-SERIAL", "cantidad": 1}]
            }),
        ))
        .await
        .expect("reserva");
    let estado: String = sqlx::query_scalar("SELECT estado FROM item.item_series")
        .fetch_one(ctx.db.pool())
        .await
        .expect("estado");
    assert_eq!(estado, "vendido");
    assert_eq!(stock(&ctx.db).await, 0);

    ctx.consumidor
        .manejar(evento(
            "devolucion.solicitada",
            Uuid::new_v4(),
            json!({
                "order_id": order_id,
                "items": [{"empresa_id": EMPRESA, "sku": "SKU-SERIAL", "cantidad": 1}]
            }),
        ))
        .await
        .expect("devolucion");
    assert_eq!(stock(&ctx.db).await, 1);
    let estado: String = sqlx::query_scalar("SELECT estado FROM item.item_series")
        .fetch_one(ctx.db.pool())
        .await
        .expect("estado");
    assert_eq!(estado, "devuelto");
    assert_eq!(
        outbox_tipos(&ctx.db).await,
        vec![
            "stock.reservado",
            "stock.updated",
            "stock.reintegrado",
            "stock.updated"
        ]
    );
}

#[tokio::test]
async fn debe_marcar_la_cotizacion_sin_tocar_stock() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-COT", 5, 1000).await;
    ctx.consumidor
        .manejar(evento(
            "order.created",
            Uuid::new_v4(),
            json!({
                "order_id": Uuid::new_v4(),
                "tipo_orden": "cotizacion",
                "items": [{"empresa_id": EMPRESA, "sku": "SKU-COT", "cantidad": 5}]
            }),
        ))
        .await
        .expect("cotizacion");
    assert_eq!(stock(&ctx.db).await, 5);
    assert!(outbox_tipos(&ctx.db).await.is_empty());
}

#[tokio::test]
async fn debe_ignorar_lineas_digitales() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-DIG", 5, 1000).await;
    ctx.consumidor
        .manejar(evento(
            "order.created",
            Uuid::new_v4(),
            json!({
                "order_id": Uuid::new_v4(),
                "items": [
                    {"empresa_id": EMPRESA, "sku": "SKU-DIG", "cantidad": 5, "fulfillment": "digital"}
                ]
            }),
        ))
        .await
        .expect("digital");
    assert_eq!(stock(&ctx.db).await, 5);
    assert_eq!(
        outbox_tipos(&ctx.db).await,
        vec!["stock.reservado", "stock.updated"]
    );
}

#[tokio::test]
async fn debe_reservar_lotes_en_orden_fefo_y_registrar_asignaciones() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-FEFO-SAGA", 10, 1000).await;
    sqlx::query(
        "INSERT INTO inventory.lotes_inventario
           (empresa_id, item_id, codigo_lote, vence_en, cantidad_inicial, cantidad_disponible)
         VALUES ($1::uuid, $2::uuid, 'L-TARDE', '2026-12-31', 10, 10),
                ($1::uuid, $2::uuid, 'L-PRONTO', '2026-11-20', 4, 4)",
    )
    .bind(EMPRESA)
    .bind(ITEM)
    .execute(ctx.db.pool())
    .await
    .expect("lotes");

    let order_id = Uuid::new_v4();
    ctx.consumidor
        .manejar(evento(
            "order.created",
            Uuid::new_v4(),
            json!({
                "order_id": order_id,
                "items": [{"empresa_id": EMPRESA, "sku": "SKU-FEFO-SAGA", "cantidad": 5}]
            }),
        ))
        .await
        .expect("reserva");

    let pronto: i32 = sqlx::query_scalar(
        "SELECT cantidad_disponible FROM inventory.lotes_inventario WHERE codigo_lote = 'L-PRONTO'",
    )
    .fetch_one(ctx.db.pool())
    .await
    .expect("pronto");
    let tarde: i32 = sqlx::query_scalar(
        "SELECT cantidad_disponible FROM inventory.lotes_inventario WHERE codigo_lote = 'L-TARDE'",
    )
    .fetch_one(ctx.db.pool())
    .await
    .expect("tarde");
    assert_eq!(pronto, 0);
    assert_eq!(tarde, 9);
    let asignaciones: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM inventory.reservas_lotes WHERE order_id = $1")
            .bind(order_id)
            .fetch_one(ctx.db.pool())
            .await
            .expect("asignaciones");
    assert_eq!(asignaciones, 2);
}

#[tokio::test]
async fn debe_expandir_componentes_bom_al_reservar() {
    let ctx = preparar().await;
    let componente = "22222222-2222-4222-8222-222222222222";
    sqlx::query(
        "INSERT INTO item.items
           (id, empresa_id, tipo, codigo, nombre, precio_cents, track_inventory, stock, estado)
         VALUES ($1::uuid, $2::uuid, 'producto', 'COMBO-BOM', 'Combo', 0, true, 2, 'activo'),
                ($3::uuid, $2::uuid, 'producto', 'COMP-BOM', 'Componente', 0, true, 10, 'activo')",
    )
    .bind("11111111-1111-4111-8111-111111111111")
    .bind(EMPRESA)
    .bind(componente)
    .execute(ctx.db.pool())
    .await
    .expect("items");
    sqlx::query(
        "INSERT INTO item.item_componentes (empresa_id, item_id, componente_item_id, cantidad)
         VALUES ($1::uuid, $2::uuid, $3::uuid, 2)",
    )
    .bind(EMPRESA)
    .bind(ITEM)
    .bind(componente)
    .execute(ctx.db.pool())
    .await
    .expect("bom");

    ctx.consumidor
        .manejar(evento(
            "order.created",
            Uuid::new_v4(),
            json!({
                "order_id": Uuid::new_v4(),
                "items": [{
                    "empresa_id": EMPRESA,
                    "sku": "COMBO-BOM",
                    "item_id": ITEM,
                    "cantidad": 2
                }]
            }),
        ))
        .await
        .expect("bom reserva");

    let stock_componente: i32 =
        sqlx::query_scalar("SELECT stock FROM item.items WHERE id = $1::uuid")
            .bind(componente)
            .fetch_one(ctx.db.pool())
            .await
            .expect("stock componente");
    assert_eq!(stock_componente, 6);
}

#[tokio::test]
async fn debe_revertir_el_stock_si_falla_el_outbox() {
    let url = url().expect("TEST_DATABASE_URL");
    let _guard = provisionar(&url).await;
    let db = db().await;
    crear_item(&db, ITEM, "SKU-ATOMICO", 10, 1000).await;
    let consumidor = PedidosConsumer::nuevo(
        Arc::clone(&db),
        Arc::new(Outbox::nueva("inventory.outbox_inexistente")),
    );
    let error = consumidor
        .manejar(evento(
            "order.created",
            Uuid::new_v4(),
            json!({
                "order_id": Uuid::new_v4(),
                "items": [{"empresa_id": EMPRESA, "sku": "SKU-ATOMICO", "cantidad": 4}]
            }),
        ))
        .await
        .expect_err("outbox");
    assert!(!error.is_empty());
    let stock: i32 = sqlx::query_scalar("SELECT stock FROM item.items WHERE id = $1::uuid")
        .bind(ITEM)
        .fetch_one(db.pool())
        .await
        .expect("stock");
    assert_eq!(stock, 10);
}
