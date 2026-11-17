//! Pruebas PostgreSQL del dominio de inventario (con `TEST_DATABASE_URL`).

mod common;

use common::*;
use inventory_service::inventario::Inventario;
use inventory_service::outbox::Outbox;
use serde_json::json;
use std::sync::Arc;

struct Contexto {
    _guard: DbLock,
    inventario: Arc<Inventario>,
    db: Arc<Db>,
}

async fn preparar() -> Contexto {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = provisionar(&url).await;
    let db = db().await;
    let outbox = Arc::new(Outbox::nueva("inventory.outbox"));
    let inventario = Arc::new(Inventario::nueva(Arc::clone(&db), Some(outbox)));
    Contexto {
        _guard: guard,
        inventario,
        db,
    }
}

#[tokio::test]
async fn debe_leer_y_guardar_la_configuracion_de_inventario() {
    let ctx = preparar().await;
    let inicial = ctx
        .inventario
        .configuracion(EMPRESA, None)
        .await
        .expect("config");
    assert_eq!(inicial["metodo_costeo"], "promedio");
    assert_eq!(inicial["permitir_stock_negativo"], false);

    let guardada = ctx
        .inventario
        .guardar_configuracion(
            EMPRESA,
            None,
            &json!({"metodo_costeo": "fifo", "permitir_stock_negativo": true}),
        )
        .await
        .expect("guardar");
    assert_eq!(guardada["metodo_costeo"], "fifo");
    assert_eq!(guardada["permitir_stock_negativo"], true);

    let releida = ctx
        .inventario
        .configuracion(EMPRESA, None)
        .await
        .expect("config");
    assert_eq!(releida["metodo_costeo"], "fifo");
}

#[tokio::test]
async fn debe_rechazar_un_metodo_de_costeo_invalido() {
    let ctx = preparar().await;
    let error = ctx
        .inventario
        .guardar_configuracion(EMPRESA, None, &json!({"metodo_costeo": "lifo"}))
        .await
        .expect_err("metodo");
    assert_eq!(error.codigo, "METODO_COSTEO_INVALIDO");
}

#[tokio::test]
async fn debe_crear_un_proveedor_con_codigo_en_mayusculas() {
    let ctx = preparar().await;
    let proveedor = ctx
        .inventario
        .crear_proveedor(&json!({
            "empresa_id": EMPRESA,
            "codigo": "prov-1",
            "nombre": "Proveedor Uno",
            "correo": "proveedor@test.dev",
            "telefono": "555-0101"
        }))
        .await
        .expect("proveedor");
    assert_eq!(proveedor["codigo"], "PROV-1");
    assert_eq!(proveedor["nombre"], "Proveedor Uno");
}

#[tokio::test]
async fn debe_consumir_lotes_en_orden_fefo_y_descontar_stock() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-FEFO", 15, 1000).await;
    ctx.inventario
        .crear_lote(&json!({
            "empresa_id": EMPRESA,
            "item_id": ITEM,
            "codigo_lote": "L-TARDE",
            "vence_en": "2026-12-31",
            "cantidad_inicial": 5
        }))
        .await
        .expect("lote tarde");
    ctx.inventario
        .crear_lote(&json!({
            "empresa_id": EMPRESA,
            "item_id": ITEM,
            "codigo_lote": "L-PRONTO",
            "vence_en": "2026-11-20",
            "cantidad_inicial": 10
        }))
        .await
        .expect("lote pronto");

    let consumo = ctx
        .inventario
        .consumir_lote(&json!({
            "empresa_id": EMPRESA,
            "item_id": ITEM,
            "cantidad": 7
        }))
        .await
        .expect("consumo");
    assert_eq!(consumo["total_consumido"], 7);
    assert_eq!(consumo["consumidos"][0]["codigo_lote"], "L-PRONTO");
    assert_eq!(consumo["consumidos"][0]["cantidad"], 7);

    let lotes = ctx
        .inventario
        .listar_lotes(EMPRESA, Some(ITEM))
        .await
        .expect("lotes");
    let pronto = lotes
        .iter()
        .find(|lote| lote["codigo_lote"] == "L-PRONTO")
        .expect("lote pronto");
    let tarde = lotes
        .iter()
        .find(|lote| lote["codigo_lote"] == "L-TARDE")
        .expect("lote tarde");
    assert_eq!(pronto["cantidad_disponible"], 3);
    assert_eq!(tarde["cantidad_disponible"], 5);

    let stock: (i32,) = sqlx::query_as("SELECT stock FROM item.items WHERE id = $1::uuid")
        .bind(ITEM)
        .fetch_one(ctx.db.pool())
        .await
        .expect("stock");
    assert_eq!(stock.0, 8);

    let evento: (String, serde_json::Value) = sqlx::query_as(
        "SELECT tipo, payload FROM inventory.outbox ORDER BY creado_en DESC LIMIT 1",
    )
    .fetch_one(ctx.db.pool())
    .await
    .expect("evento stock.updated");
    assert_eq!(evento.0, "stock.updated");
    assert_eq!(evento.1["data"]["items"][0]["sku"], "SKU-FEFO");
    assert_eq!(evento.1["data"]["items"][0]["stock_restante"], 8);
}

#[tokio::test]
async fn debe_fallar_si_los_lotes_no_alcanzan_y_rechazar_vencidos() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-FEFO-2", 10, 1000).await;
    ctx.inventario
        .crear_lote(&json!({
            "empresa_id": EMPRESA,
            "item_id": ITEM,
            "codigo_lote": "L-VENCIDO",
            "vence_en": "2020-01-01",
            "cantidad_inicial": 5
        }))
        .await
        .expect("lote vencido");

    let error = ctx
        .inventario
        .consumir_lote(&json!({
            "empresa_id": EMPRESA,
            "item_id": ITEM,
            "cantidad": 1,
            "codigo_lote": "L-VENCIDO"
        }))
        .await
        .expect_err("vencido");
    assert_eq!(error.codigo, "LOTE_VENCIDO");

    let error = ctx
        .inventario
        .consumir_lote(&json!({
            "empresa_id": EMPRESA,
            "item_id": ITEM,
            "cantidad": 1
        }))
        .await
        .expect_err("sin lotes vigentes");
    assert_eq!(error.codigo, "STOCK_LOTE_INSUFICIENTE");
}

#[tokio::test]
async fn debe_listar_stock_fisico_reservado_y_valorizado() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-LISTA", 10, 1000).await;
    sqlx::query(
        "INSERT INTO inventory.reservas_ordenes (order_id, items_json, creado_en)
         VALUES (gen_random_uuid(),
                 $1::jsonb,
                 NOW())",
    )
    .bind(json!([{"empresa_id": EMPRESA, "sku": "SKU-LISTA", "cantidad": 2}]))
    .execute(ctx.db.pool())
    .await
    .expect("reserva");

    let lineas = ctx
        .inventario
        .listar(Some(EMPRESA), None)
        .await
        .expect("lineas");
    assert_eq!(lineas.len(), 1);
    let linea = &lineas[0];
    assert_eq!(linea["sku"], "SKU-LISTA");
    assert_eq!(linea["stock"], "12");
    assert_eq!(linea["disponible"], 10);
    assert_eq!(linea["reservado"], 2);
    assert_eq!(linea["precio_base"], "10.00");
    assert_eq!(linea["valor_disponible_cents"], 10_000);
}

#[tokio::test]
async fn debe_resumir_productos_agotados_y_valor_del_inventario() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-UNO", 10, 1000).await;
    crear_item(
        &ctx.db,
        "22222222-2222-4222-8222-222222222222",
        "SKU-DOS",
        0,
        500,
    )
    .await;

    let resumen = ctx
        .inventario
        .resumen(Some(EMPRESA), None)
        .await
        .expect("resumen");
    assert_eq!(resumen["productos"], 2);
    assert_eq!(resumen["agotados"], 1);
    assert_eq!(resumen["stock_total"], 10);
    assert_eq!(resumen["valor_inventario_cents"], 10_000);
}

#[tokio::test]
async fn debe_entregar_stock_y_series_por_lote_de_skus() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-INTERNO", 10, 1000).await;

    let lineas = ctx
        .inventario
        .stock_lote(EMPRESA, &["sku-interno".to_string()])
        .await
        .expect("stock lote");
    assert_eq!(lineas.len(), 1);
    assert_eq!(lineas[0]["sku"], "SKU-INTERNO");
    assert_eq!(lineas[0]["empresa_id"], EMPRESA);
    assert_eq!(lineas[0]["stock"], 10);
    assert_eq!(lineas[0]["seriales_disponibles"], 0);
}

#[tokio::test]
async fn debe_filtrar_inventario_por_negocio() {
    let ctx = preparar().await;
    let negocio = "22222222-2222-4222-8222-222222222222";
    crear_item(&ctx.db, ITEM, "SKU-NEGOCIO", 5, 1000).await;
    sqlx::query("UPDATE item.items SET negocio_id = $1::uuid WHERE id = $2::uuid")
        .bind(negocio)
        .bind(ITEM)
        .execute(ctx.db.pool())
        .await
        .expect("negocio");

    let lineas = ctx
        .inventario
        .listar(Some(EMPRESA), Some(negocio))
        .await
        .expect("listar negocio");
    assert_eq!(lineas.len(), 1);
    assert_eq!(lineas[0]["sku"], "SKU-NEGOCIO");

    let resumen = ctx
        .inventario
        .resumen(Some(EMPRESA), Some(negocio))
        .await
        .expect("resumen negocio");
    assert_eq!(resumen["productos"], 1);

    let otro = ctx
        .inventario
        .listar(Some(EMPRESA), Some("33333333-3333-4333-8333-333333333333"))
        .await
        .expect("listar otro");
    assert!(otro.is_empty());

    let configuracion = ctx
        .inventario
        .configuracion(EMPRESA, Some(negocio))
        .await
        .expect("configuracion negocio");
    assert_eq!(configuracion["negocio_id"], negocio);
}

#[tokio::test]
async fn debe_validar_el_lote_de_stock() {
    let ctx = preparar().await;
    let vacio = ctx
        .inventario
        .stock_lote(EMPRESA, &[])
        .await
        .expect("vacio");
    assert!(vacio.is_empty());

    let error = ctx
        .inventario
        .stock_lote("no-es-uuid", &["SKU-1".to_string()])
        .await
        .expect_err("empresa invalida");
    assert_eq!(error.codigo, "VALIDACION");

    let inexistente = ctx
        .inventario
        .stock_lote(EMPRESA, &["NO-EXISTE".to_string()])
        .await
        .expect("inexistente");
    assert!(inexistente.is_empty());
}
