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

    let stock: (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(stock), 0)::bigint
           FROM inventory.item_stock_ubicacion WHERE item_id = $1::uuid",
    )
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
}

#[tokio::test]
async fn debe_entregar_stock_y_series_por_lote_de_skus() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-INTERNO", 10, 1000).await;

    let lineas = ctx
        .inventario
        .stock_lote(EMPRESA, &["sku-interno".to_string()], None)
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
        .stock_lote(EMPRESA, &[], None)
        .await
        .expect("vacio");
    assert!(vacio.is_empty());

    let error = ctx
        .inventario
        .stock_lote("no-es-uuid", &["SKU-1".to_string()], None)
        .await
        .expect_err("empresa invalida");
    assert_eq!(error.codigo, "VALIDACION");

    let inexistente = ctx
        .inventario
        .stock_lote(EMPRESA, &["NO-EXISTE".to_string()], None)
        .await
        .expect("inexistente");
    assert!(inexistente.is_empty());
}

#[tokio::test]
async fn debe_registrar_stock_inicial_en_la_ubicacion_predeterminada() {
    let ctx = preparar().await;
    sqlx::query(
        "INSERT INTO item.items (id, empresa_id, tipo, codigo, nombre, track_inventory, estado)
         VALUES ($1::uuid, $2::uuid, 'producto', 'SKU-INICIAL', 'Inicial', true, 'activo')",
    )
    .bind(ITEM)
    .bind(EMPRESA)
    .execute(ctx.db.pool())
    .await
    .expect("item");

    let respuesta = ctx
        .inventario
        .stock_inicial(EMPRESA, ITEM, "SKU-INICIAL", 7, None)
        .await
        .expect("stock inicial");
    assert_eq!(respuesta["stock"], 7);

    let total: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(stock), 0)::bigint
           FROM inventory.item_stock_ubicacion WHERE item_id = $1::uuid",
    )
    .bind(ITEM)
    .fetch_one(ctx.db.pool())
    .await
    .expect("total");
    assert_eq!(total, 7);
}

#[tokio::test]
async fn debe_validar_y_actualizar_el_stock_inicial() {
    let ctx = preparar().await;
    let error = ctx
        .inventario
        .stock_inicial(EMPRESA, ITEM, "SKU", -1, None)
        .await
        .expect_err("cantidad");
    assert_eq!(error.codigo, "VALIDACION");

    sqlx::query(
        "INSERT INTO item.items (id, empresa_id, tipo, codigo, nombre, track_inventory, estado)
         VALUES ($1::uuid, $2::uuid, 'producto', 'SKU-INICIAL-2', 'Inicial 2', true, 'activo'),
                ($3::uuid, $2::uuid, 'producto', 'SERV-INICIAL', 'Servicio', false, 'activo')",
    )
    .bind(ITEM)
    .bind(EMPRESA)
    .bind("22222222-2222-4222-8222-222222222222")
    .execute(ctx.db.pool())
    .await
    .expect("items");

    let error = ctx
        .inventario
        .stock_inicial(
            EMPRESA,
            "22222222-2222-4222-8222-222222222222",
            "SERV-INICIAL",
            1,
            None,
        )
        .await
        .expect_err("no inventario");
    assert_eq!(error.codigo, "INVENTARIO_NO_APLICA");

    let error = ctx
        .inventario
        .stock_inicial(
            EMPRESA,
            "33333333-3333-4333-8333-333333333333",
            "X",
            1,
            None,
        )
        .await
        .expect_err("no existe");
    assert_eq!(error.codigo, "NO_ENCONTRADO");

    ctx.inventario
        .stock_inicial(EMPRESA, ITEM, "SKU", 5, None)
        .await
        .expect("inicial");
    ctx.inventario
        .stock_inicial(EMPRESA, ITEM, "SKU", 9, None)
        .await
        .expect("actualizar");
    let total: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(stock), 0)::bigint
           FROM inventory.item_stock_ubicacion WHERE item_id = $1::uuid",
    )
    .bind(ITEM)
    .fetch_one(ctx.db.pool())
    .await
    .expect("total");
    assert_eq!(total, 9);
}

#[tokio::test]
async fn debe_aplicar_deltas_de_stock_en_ubicaciones() {
    let ctx = preparar().await;
    sqlx::query(
        "INSERT INTO item.items (id, empresa_id, tipo, codigo, nombre, track_inventory, estado)
         VALUES ($1::uuid, $2::uuid, 'producto', 'SKU-DELTA', 'Delta', true, 'activo')",
    )
    .bind(ITEM)
    .bind(EMPRESA)
    .execute(ctx.db.pool())
    .await
    .expect("item");

    let mut tx = ctx.db.transaccion().await.expect("tx");
    let error = inventory_service::stock::aplicar_delta_en_transaccion(&mut tx, EMPRESA, ITEM, 2)
        .await
        .expect_err("sin ubicacion");
    assert_eq!(error.codigo, "SIN_UBICACION");
    tx.rollback().await.expect("rollback");

    ctx.inventario
        .stock_inicial(EMPRESA, ITEM, "SKU-DELTA", 10, None)
        .await
        .expect("inicial");

    let mut tx = ctx.db.transaccion().await.expect("tx");
    let total = inventory_service::stock::aplicar_delta_en_transaccion(&mut tx, EMPRESA, ITEM, -3)
        .await
        .expect("salida");
    assert_eq!(total, 7);
    let error = inventory_service::stock::aplicar_delta_en_transaccion(&mut tx, EMPRESA, ITEM, -10)
        .await
        .expect_err("insuficiente");
    assert_eq!(error.codigo, "STOCK_INSUFICIENTE");
    tx.rollback().await.expect("rollback");
}

#[tokio::test]
async fn debe_reservar_stock_en_la_ubicacion_elegida() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-LOC", 10, 1000).await;
    let segunda = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO inventory.ubicaciones
           (id, empresa_id, codigo, nombre, predeterminada, activo)
         VALUES ($1::uuid, $2::uuid, 'sucursal', 'Sucursal', false, true)",
    )
    .bind(segunda)
    .bind(EMPRESA)
    .execute(ctx.db.pool())
    .await
    .expect("ubicacion secundaria");
    sqlx::query(
        "INSERT INTO inventory.item_stock_ubicacion
           (empresa_id, item_id, ubicacion_id, stock)
         VALUES ($1::uuid, $2::uuid, $3::uuid, 4)",
    )
    .bind(EMPRESA)
    .bind(ITEM)
    .bind(segunda)
    .execute(ctx.db.pool())
    .await
    .expect("stock secundario");

    let lineas = json!([{
        "empresa_id": EMPRESA,
        "sku": "SKU-LOC",
        "cantidad": 3,
        "item_id": ITEM,
        "ubicacion_id": segunda.to_string(),
    }]);
    let mut tx = ctx.db.transaccion().await.expect("tx");
    let reserva = inventory_service::reservas::Reservas::reservar_en_transaccion(
        &mut tx,
        "77777777-7777-4777-8777-777777777777",
        &lineas,
    )
    .await
    .expect("reserva");
    assert_eq!(reserva["ok"], true);
    tx.commit().await.expect("commit");

    let stock_segunda: i64 = sqlx::query_scalar(
        "SELECT stock::bigint FROM inventory.item_stock_ubicacion
          WHERE item_id = $1::uuid AND ubicacion_id = $2::uuid",
    )
    .bind(ITEM)
    .bind(segunda)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock segunda");
    assert_eq!(stock_segunda, 1);

    let default: uuid::Uuid = sqlx::query_scalar(
        "SELECT id FROM inventory.ubicaciones
          WHERE empresa_id = $1::uuid AND predeterminada LIMIT 1",
    )
    .bind(EMPRESA)
    .fetch_one(ctx.db.pool())
    .await
    .expect("default");
    let stock_default: i64 = sqlx::query_scalar(
        "SELECT stock::bigint FROM inventory.item_stock_ubicacion
          WHERE item_id = $1::uuid AND ubicacion_id = $2::uuid",
    )
    .bind(ITEM)
    .bind(default)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock default");
    assert_eq!(stock_default, 10);
}

#[tokio::test]
async fn debe_rechazar_la_reserva_si_la_ubicacion_no_alcanza() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-LOC-CORTA", 10, 1000).await;
    let segunda = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO inventory.ubicaciones
           (id, empresa_id, codigo, nombre, predeterminada, activo)
         VALUES ($1::uuid, $2::uuid, 'sucursal', 'Sucursal', false, true)",
    )
    .bind(segunda)
    .bind(EMPRESA)
    .execute(ctx.db.pool())
    .await
    .expect("ubicacion secundaria");
    sqlx::query(
        "INSERT INTO inventory.item_stock_ubicacion
           (empresa_id, item_id, ubicacion_id, stock)
         VALUES ($1::uuid, $2::uuid, $3::uuid, 1)",
    )
    .bind(EMPRESA)
    .bind(ITEM)
    .bind(segunda)
    .execute(ctx.db.pool())
    .await
    .expect("stock secundario");

    let lineas = json!([{
        "empresa_id": EMPRESA,
        "sku": "SKU-LOC-CORTA",
        "cantidad": 2,
        "item_id": ITEM,
        "ubicacion_id": segunda.to_string(),
    }]);
    let mut tx = ctx.db.transaccion().await.expect("tx");
    let reserva = inventory_service::reservas::Reservas::reservar_en_transaccion(
        &mut tx,
        "88888888-8888-4888-8888-888888888888",
        &lineas,
    )
    .await
    .expect("reserva");
    assert_eq!(reserva["ok"], false);
    tx.rollback().await.expect("rollback");
}

#[tokio::test]
async fn debe_devolver_totales_vacios_para_una_empresa_sin_items() {
    let ctx = preparar().await;
    let otra = "99999999-9999-4999-8999-999999999999";
    let lineas = ctx
        .inventario
        .listar(Some(otra), None)
        .await
        .expect("listar");
    assert!(lineas.is_empty());
    let resumen = ctx
        .inventario
        .resumen(Some(otra), None)
        .await
        .expect("resumen");
    assert_eq!(resumen["productos"], 0);
    assert_eq!(resumen["stock_total"], 0);
}

#[tokio::test]
async fn debe_reintegrar_stock_parcial_de_una_devolucion_una_sola_vez() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-RMA", 10, 1000).await;
    let devolucion = "99999999-9999-4999-8999-999999999999";
    let order = "88888888-8888-4888-8888-888888888888";
    let items = json!([{
        "empresa_id": EMPRESA,
        "sku": "SKU-RMA",
        "cantidad": 2,
        "item_id": ITEM,
    }]);

    let mut tx = ctx.db.transaccion().await.expect("tx");
    inventory_service::reservas::Reservas::reintegrar_lineas_en_transaccion(
        &mut tx, order, devolucion, &items,
    )
    .await
    .expect("reintegrar");
    tx.commit().await.expect("commit");

    let stock: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(stock), 0)::bigint FROM inventory.item_stock_ubicacion
          WHERE item_id = $1::uuid",
    )
    .bind(ITEM)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock");
    assert_eq!(stock, 12);

    let mut tx = ctx.db.transaccion().await.expect("tx");
    inventory_service::reservas::Reservas::reintegrar_lineas_en_transaccion(
        &mut tx, order, devolucion, &items,
    )
    .await
    .expect("idempotente");
    tx.commit().await.expect("commit");
    let stock: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(stock), 0)::bigint FROM inventory.item_stock_ubicacion
          WHERE item_id = $1::uuid",
    )
    .bind(ITEM)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock idempotente");
    assert_eq!(stock, 12);
}

#[tokio::test]
async fn debe_reintegrar_la_devolucion_en_la_ubicacion_elegida() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-RMA-LOC", 10, 1000).await;
    let segunda = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO inventory.ubicaciones
           (id, empresa_id, codigo, nombre, predeterminada, activo)
         VALUES ($1::uuid, $2::uuid, 'sucursal', 'Sucursal', false, true)",
    )
    .bind(segunda)
    .bind(EMPRESA)
    .execute(ctx.db.pool())
    .await
    .expect("ubicacion");
    sqlx::query(
        "INSERT INTO inventory.item_stock_ubicacion
           (empresa_id, item_id, ubicacion_id, stock)
         VALUES ($1::uuid, $2::uuid, $3::uuid, 5)",
    )
    .bind(EMPRESA)
    .bind(ITEM)
    .bind(segunda)
    .execute(ctx.db.pool())
    .await
    .expect("stock ubicacion");

    let items = json!([{
        "empresa_id": EMPRESA,
        "sku": "SKU-RMA-LOC",
        "cantidad": 2,
        "item_id": ITEM,
        "ubicacion_id": segunda.to_string(),
    }]);
    let mut tx = ctx.db.transaccion().await.expect("tx");
    inventory_service::reservas::Reservas::reintegrar_lineas_en_transaccion(
        &mut tx,
        "77777777-7777-4777-8777-777777777777",
        "66666666-6666-4666-8666-666666666666",
        &items,
    )
    .await
    .expect("reintegrar");
    tx.commit().await.expect("commit");

    let stock: i64 = sqlx::query_scalar(
        "SELECT stock::bigint FROM inventory.item_stock_ubicacion
          WHERE item_id = $1::uuid AND ubicacion_id = $2::uuid",
    )
    .bind(ITEM)
    .bind(segunda)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock");
    assert_eq!(stock, 7);
}

#[tokio::test]
async fn debe_reintegrar_lotes_reservados_en_una_devolucion() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-RMA-LOTE", 10, 1000).await;
    let lote = uuid::Uuid::new_v4();
    let order = "55555555-5555-4555-8555-555555555555";
    sqlx::query(
        "INSERT INTO inventory.lotes_inventario
           (id, empresa_id, item_id, codigo_lote, cantidad_inicial, cantidad_disponible)
         VALUES ($1::uuid, $2::uuid, $3::uuid, 'L-RMA', 20, 10)",
    )
    .bind(lote)
    .bind(EMPRESA)
    .bind(ITEM)
    .execute(ctx.db.pool())
    .await
    .expect("lote");
    sqlx::query(
        "INSERT INTO inventory.reservas_lotes
           (order_id, lote_id, empresa_id, item_id, cantidad)
         VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, 4)",
    )
    .bind(order)
    .bind(lote)
    .bind(EMPRESA)
    .bind(ITEM)
    .execute(ctx.db.pool())
    .await
    .expect("reserva lote");

    let items = json!([{
        "empresa_id": EMPRESA,
        "sku": "SKU-RMA-LOTE",
        "cantidad": 3,
        "item_id": ITEM,
    }]);
    let mut tx = ctx.db.transaccion().await.expect("tx");
    inventory_service::reservas::Reservas::reintegrar_lineas_en_transaccion(
        &mut tx,
        order,
        "44444444-4444-4444-8444-444444444444",
        &items,
    )
    .await
    .expect("reintegrar");
    tx.commit().await.expect("commit");

    let disponible: i32 = sqlx::query_scalar(
        "SELECT cantidad_disponible FROM inventory.lotes_inventario WHERE id = $1::uuid",
    )
    .bind(lote)
    .fetch_one(ctx.db.pool())
    .await
    .expect("lote");
    assert_eq!(disponible, 13);
    let reservado: i32 = sqlx::query_scalar(
        "SELECT cantidad FROM inventory.reservas_lotes WHERE order_id = $1::uuid AND lote_id = $2::uuid",
    )
    .bind(uuid::Uuid::parse_str(order).expect("uuid"))
    .bind(lote)
    .fetch_one(ctx.db.pool())
    .await
    .expect("reserva");
    assert_eq!(reservado, 1);
}

#[tokio::test]
async fn debe_ignorar_lineas_inexistentes_en_una_devolucion() {
    let ctx = preparar().await;
    let items = json!([{
        "empresa_id": EMPRESA,
        "sku": "NO-EXISTE-RMA",
        "cantidad": 1,
    }]);
    let mut tx = ctx.db.transaccion().await.expect("tx");
    inventory_service::reservas::Reservas::reintegrar_lineas_en_transaccion(
        &mut tx,
        "33333333-3333-4333-8333-333333333333",
        "22222222-2222-4222-8222-222222222222",
        &items,
    )
    .await
    .expect("sin item");
    tx.commit().await.expect("commit");
}

#[tokio::test]
async fn debe_reintegrar_series_vendidas_en_una_devolucion() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-RMA-SERIE", 2, 1000).await;
    let order = "11111111-2222-4333-8444-555555555555";
    let serie = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO item.item_series
           (id, empresa_id, item_id, serial, estado, order_id, vendido_en)
         VALUES ($1::uuid, $2::uuid, $3::uuid, 'SERIE-1', 'vendido', $4::uuid, NOW())",
    )
    .bind(serie)
    .bind(EMPRESA)
    .bind(ITEM)
    .bind(order)
    .execute(ctx.db.pool())
    .await
    .expect("serie");

    let items = json!([{
        "empresa_id": EMPRESA,
        "sku": "SKU-RMA-SERIE",
        "cantidad": 1,
        "item_id": ITEM,
    }]);
    let mut tx = ctx.db.transaccion().await.expect("tx");
    inventory_service::reservas::Reservas::reintegrar_lineas_en_transaccion(
        &mut tx,
        order,
        "11111111-3333-4444-8555-666666666666",
        &items,
    )
    .await
    .expect("reintegrar");
    tx.commit().await.expect("commit");

    let estado: String =
        sqlx::query_scalar("SELECT estado FROM item.item_series WHERE id = $1::uuid")
            .bind(serie)
            .fetch_one(ctx.db.pool())
            .await
            .expect("serie");
    assert_eq!(estado, "disponible");
}

#[tokio::test]
async fn debe_reintegrar_la_orden_completa_en_la_ubicacion() {
    let ctx = preparar().await;
    crear_item(&ctx.db, ITEM, "SKU-FULL-LOC", 10, 1000).await;
    let segunda = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO inventory.ubicaciones
           (id, empresa_id, codigo, nombre, predeterminada, activo)
         VALUES ($1::uuid, $2::uuid, 'sucursal', 'Sucursal', false, true)",
    )
    .bind(segunda)
    .bind(EMPRESA)
    .execute(ctx.db.pool())
    .await
    .expect("ubicacion");
    sqlx::query(
        "INSERT INTO inventory.item_stock_ubicacion
           (empresa_id, item_id, ubicacion_id, stock)
         VALUES ($1::uuid, $2::uuid, $3::uuid, 5)",
    )
    .bind(EMPRESA)
    .bind(ITEM)
    .bind(segunda)
    .execute(ctx.db.pool())
    .await
    .expect("stock");

    let items = json!([{
        "empresa_id": EMPRESA,
        "sku": "SKU-FULL-LOC",
        "cantidad": 2,
        "item_id": ITEM,
        "ubicacion_id": segunda.to_string(),
    }]);
    let orden = "99999999-1111-4222-8333-444444444444";

    let mut tx = ctx.db.transaccion().await.expect("tx");
    let reserva =
        inventory_service::reservas::Reservas::reservar_en_transaccion(&mut tx, orden, &items)
            .await
            .expect("reservar");
    assert_eq!(reserva["ok"], true);
    tx.commit().await.expect("commit");

    let stock_reservado: i64 = sqlx::query_scalar(
        "SELECT stock::bigint FROM inventory.item_stock_ubicacion
          WHERE item_id = $1::uuid AND ubicacion_id = $2::uuid",
    )
    .bind(ITEM)
    .bind(segunda)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock");
    assert_eq!(stock_reservado, 3);

    let mut tx = ctx.db.transaccion().await.expect("tx");
    let liberado =
        inventory_service::reservas::Reservas::reintegrar_en_transaccion(&mut tx, orden, &items)
            .await
            .expect("reintegrar");
    assert!(liberado);
    tx.commit().await.expect("commit");

    let stock: i64 = sqlx::query_scalar(
        "SELECT stock::bigint FROM inventory.item_stock_ubicacion
          WHERE item_id = $1::uuid AND ubicacion_id = $2::uuid",
    )
    .bind(ITEM)
    .bind(segunda)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock");
    assert_eq!(stock, 5);

    let mut tx = ctx.db.transaccion().await.expect("tx");
    let segunda_liberacion =
        inventory_service::reservas::Reservas::reintegrar_en_transaccion(&mut tx, orden, &items)
            .await
            .expect("reintegrar idempotente");
    assert!(!segunda_liberacion);
    tx.commit().await.expect("commit");
}
