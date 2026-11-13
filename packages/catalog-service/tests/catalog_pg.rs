//! Pruebas PostgreSQL del dominio de catálogo (con `TEST_DATABASE_URL`).

mod common;

use catalog_service::componentes::Componentes;
use catalog_service::consumidor::{cola, PedidosConsumer};
use catalog_service::inventario::Inventario;
use catalog_service::items::Items;
use catalog_service::outbox::Outbox;
use catalog_service::precios::Precios;
use catalog_service::productos::Productos;
use catalog_service::series::Series;
use catalog_service::tarifas::Tarifas;
use catalog_service::ubicaciones::Ubicaciones;
use common::*;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

struct Contexto {
    _guard: tokio::sync::MutexGuard<'static, ()>,
    db: Arc<Db>,
    items: Arc<Items>,
    productos: Arc<Productos>,
    outbox: Arc<Outbox>,
    precios: Arc<Precios>,
    tarifas: Arc<Tarifas>,
    series: Arc<Series>,
    componentes: Arc<Componentes>,
    inventario: Arc<Inventario>,
    ubicaciones: Arc<Ubicaciones>,
}

async fn preparar() -> Contexto {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = db().await;
    let outbox = Arc::new(Outbox::nueva("catalog.outbox"));
    let items = Arc::new(Items::nueva(Arc::clone(&db)));
    let productos = Arc::new(Productos::nueva(
        Arc::clone(&db),
        Some(Arc::clone(&outbox)),
        Some(Arc::clone(&items)),
    ));
    Contexto {
        _guard: guard,
        precios: Arc::new(Precios::nueva(Arc::clone(&db))),
        tarifas: Arc::new(Tarifas::nueva(Arc::clone(&db))),
        series: Arc::new(Series::nueva(Arc::clone(&db))),
        componentes: Arc::new(Componentes::nueva(Arc::clone(&db), Arc::clone(&items))),
        inventario: Arc::new(Inventario::nueva(Arc::clone(&db))),
        ubicaciones: Arc::new(Ubicaciones::nueva(Arc::clone(&db), Arc::clone(&items))),
        db,
        items,
        productos,
        outbox,
    }
}

async fn crear_item(ctx: &Contexto, codigo: &str, precio: &str, stock: i64) -> serde_json::Value {
    ctx.items
        .crear_item(&payload_item(codigo, precio, stock), EMPRESA)
        .await
        .expect("item")
}

#[tokio::test]
async fn debe_crear_listar_y_validar_atributos_json_schema() {
    let ctx = preparar().await;
    let item = crear_item(&ctx, "SKU-1", "1000.00", 5).await;
    assert_eq!(item["codigo"], "SKU-1");
    assert_eq!(item["precio"], "1000.00");
    assert_eq!(item["stock"], 5);
    assert_eq!(item["track_inventory"], true);

    let pagina = ctx
        .items
        .listar(&json!({"empresa_id": EMPRESA, "limite": "10"}))
        .await
        .expect("listar");
    assert_eq!(pagina["total"], 1);
    assert_eq!(pagina["items"][0]["item_id"], item["item_id"]);

    let encontrado = ctx
        .items
        .encontrar_por_id(item["item_id"].as_str().expect("id"), Some(EMPRESA))
        .await
        .expect("detalle")
        .expect("existe");
    assert_eq!(encontrado["nombre"], "Producto de prueba");

    let duplicado = ctx
        .items
        .crear_item(&payload_item("SKU-1", "1000.00", 1), EMPRESA)
        .await
        .expect_err("duplicado");
    assert_eq!(duplicado.status, axum::http::StatusCode::CONFLICT);

    ctx.items
        .guardar_esquema(
            Some(EMPRESA),
            &json!("producto"),
            "farmacia",
            &json!({
                "type": "object",
                "properties": {"laboratorio": {"type": "string"}},
                "required": ["laboratorio"],
                "additionalProperties": false
            }),
        )
        .await
        .expect("esquema");
    let invalido = ctx
        .items
        .crear_item(
            &json!({
                "tipo": "producto",
                "codigo": "SKU-2",
                "nombre": "Otro",
                "precio": "10.00",
                "stock": 1,
                "categoria": "farmacia",
                "atributos": {"otro": true}
            }),
            EMPRESA,
        )
        .await
        .expect_err("atributos");
    assert_eq!(invalido.codigo, "ATRIBUTOS_INVALIDOS");

    let valido = ctx
        .items
        .crear_item(
            &json!({
                "tipo": "producto",
                "codigo": "SKU-3",
                "nombre": "Valido",
                "precio": "10.00",
                "stock": 1,
                "categoria": "farmacia",
                "atributos": {"laboratorio": "QA"}
            }),
            EMPRESA,
        )
        .await
        .expect("valido");
    assert_eq!(valido["atributos"]["laboratorio"], "QA");
}

#[tokio::test]
async fn debe_reservar_reintegrar_y_consumir_pedidos_idempotentes() {
    let ctx = preparar().await;
    crear_item(&ctx, "SKU-STOCK", "1000.00", 10).await;
    let lineas = json!([{
        "empresa_id": EMPRESA,
        "sku": "SKU-STOCK",
        "cantidad": 3
    }]);
    let reserva = ctx
        .productos
        .reservar_stock("11111111-1111-4111-8111-111111111111", &lineas)
        .await
        .expect("reserva");
    assert_eq!(reserva["ok"], true);
    assert_eq!(reserva["stock_restante"][0]["stock_restante"], 7);

    ctx.productos
        .reintegrar_stock("11111111-1111-4111-8111-111111111111", &lineas)
        .await
        .expect("reintegrar");
    let stock: i32 = sqlx::query_scalar(
        "SELECT stock FROM catalog.items WHERE empresa_id = $1::uuid AND codigo = 'SKU-STOCK'",
    )
    .bind(EMPRESA)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock");
    assert_eq!(stock, 10);

    let consumidor = Arc::new(PedidosConsumer::nuevo(
        Arc::clone(&ctx.db),
        Arc::clone(&ctx.outbox),
        Arc::clone(&ctx.productos),
    ));
    let evento = json!({
        "event_id": "22222222-2222-4222-8222-222222222222",
        "tipo": "order.created",
        "data": {
            "order_id": "33333333-3333-4333-8333-333333333333",
            "tipo_orden": "venta",
            "items": [{
                "empresa_id": EMPRESA,
                "sku": "SKU-STOCK",
                "cantidad": 2,
                "fulfillment": "pickup"
            }]
        }
    });
    consumidor.manejar(evento.clone()).await.expect("consumo");
    let stock: i32 = sqlx::query_scalar(
        "SELECT stock FROM catalog.items WHERE empresa_id = $1::uuid AND codigo = 'SKU-STOCK'",
    )
    .bind(EMPRESA)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock");
    assert_eq!(stock, 8);

    consumidor.manejar(evento).await.expect("reintento");
    let stock: i32 = sqlx::query_scalar(
        "SELECT stock FROM catalog.items WHERE empresa_id = $1::uuid AND codigo = 'SKU-STOCK'",
    )
    .bind(EMPRESA)
    .fetch_one(ctx.db.pool())
    .await
    .expect("stock");
    assert_eq!(stock, 8);
    assert_eq!(cola().nombre, "catalog.pedidos");
}

#[tokio::test]
async fn debe_resolver_precios_escalas_y_listas() {
    let ctx = preparar().await;
    let item = crear_item(&ctx, "SKU-PRECIO", "1000.00", 5).await;
    let item_id = item["item_id"].as_str().expect("id");
    let lista = ctx
        .precios
        .crear_lista(
            EMPRESA,
            &json!({"codigo": "mayorista", "nombre": "Mayorista", "moneda": "NIO"}),
        )
        .await
        .expect("lista");
    let lista_id = lista["id"].as_str().expect("lista id");
    ctx.precios
        .crear_precio(
            EMPRESA,
            Some(lista_id),
            &json!({"item_id": item_id, "desde_cantidad": 1, "precio_cents": 90000}),
        )
        .await
        .expect("precio lista");
    ctx.precios
        .crear_escala(
            EMPRESA,
            item_id,
            &json!({"desde_cantidad": 10, "hasta_cantidad": null, "precio_cents": 80000}),
        )
        .await
        .expect("escala");

    let resuelto = ctx
        .precios
        .precio_de(
            EMPRESA,
            item_id,
            &json!("1"),
            &json!({"lista_codigo": "mayorista"}),
        )
        .await
        .expect("precio");
    assert_eq!(resuelto["origen"], "lista");
    assert_eq!(resuelto["precio_unitario_cents"], 90000);

    let escala = ctx
        .precios
        .precio_de(EMPRESA, item_id, &json!("12"), &json!({}))
        .await
        .expect("escala");
    assert_eq!(escala["origen"], "escala");
    assert_eq!(escala["precio_unitario_cents"], 80000);
    assert_eq!(escala["precio_total_cents"], 960000);

    let precios = ctx
        .precios
        .crear_precio(
            EMPRESA,
            None,
            &json!({"item_id": item_id, "desde_cantidad": 5, "hasta_cantidad": 9, "precio_cents": 70000}),
        )
        .await
        .expect("escala directa");
    assert_eq!(precios["precio_cents"], 70000);
}

#[tokio::test]
async fn debe_cotizar_tarifas_por_rango() {
    let ctx = preparar().await;
    let item = crear_item(&ctx, "SKU-TARIFA", "1000.00", 1).await;
    let item_id = item["item_id"].as_str().expect("id");
    ctx.tarifas
        .crear(
            EMPRESA,
            item_id,
            &json!({"desde": "2026-12-20", "hasta": "2026-12-22", "precio_cents": 20000}),
        )
        .await
        .expect("tarifa");
    let cotizacion = ctx
        .tarifas
        .cotizacion(EMPRESA, item_id, &json!("2026-12-19"), &json!("2026-12-23"))
        .await
        .expect("cotizacion");
    assert_eq!(cotizacion["dias"], 5);
    assert_eq!(cotizacion["total_cents"], 260000);
    assert_eq!(
        cotizacion["desglose"].as_array().expect("desglose").len(),
        3
    );
}

#[tokio::test]
async fn debe_gestionar_series_y_componentes() {
    let ctx = preparar().await;
    let padre = crear_item(&ctx, "SKU-PADRE", "1000.00", 5).await;
    let padre_id = padre["item_id"].as_str().expect("id");
    let componente = crear_item(&ctx, "SKU-COMP", "500.00", 10).await;
    let componente_id = componente["item_id"].as_str().expect("id");

    ctx.series
        .habilitar_serializacion(EMPRESA, padre_id, true)
        .await
        .expect("serializacion");
    let serie = ctx
        .series
        .crear(
            EMPRESA,
            padre_id,
            &json!({"serial": "SN-001", "imei": "356789012345678"}),
        )
        .await
        .expect("serie");
    assert_eq!(serie["serial"], "SN-001");
    let series = ctx.series.listar(EMPRESA, padre_id).await.expect("listar");
    assert_eq!(series.as_array().expect("series").len(), 1);

    ctx.componentes
        .crear(
            EMPRESA,
            padre_id,
            &json!({"componente_item_id": componente_id, "cantidad": 2}),
        )
        .await
        .expect("componente");
    let ciclo = ctx
        .componentes
        .crear(
            EMPRESA,
            componente_id,
            &json!({"componente_item_id": padre_id, "cantidad": 1}),
        )
        .await
        .expect_err("ciclo");
    assert!(ciclo.codigo == "COMPONENTE_INVALIDO" || ciclo.codigo == "CONFLICTO");
}

#[tokio::test]
async fn debe_consumir_lotes_fefo_y_transferir_stock() {
    let ctx = preparar().await;
    let item = crear_item(&ctx, "SKU-LOTE", "1000.00", 10).await;
    let item_id = item["item_id"].as_str().expect("id");
    let proveedor = ctx
        .inventario
        .crear_proveedor(&json!({
            "empresa_id": EMPRESA,
            "codigo": "PROV-1",
            "nombre": "Proveedor QA"
        }))
        .await
        .expect("proveedor");
    assert_eq!(proveedor["codigo"], "PROV-1");
    ctx.inventario
        .crear_lote(&json!({
            "empresa_id": EMPRESA,
            "item_id": item_id,
            "codigo_lote": "LOTE-1",
            "vence_en": "2027-01-01",
            "cantidad_inicial": 5
        }))
        .await
        .expect("lote");
    let consumo = ctx
        .inventario
        .consumir_lote(&json!({
            "empresa_id": EMPRESA,
            "item_id": item_id,
            "cantidad": 2
        }))
        .await
        .expect("consumo");
    assert!(consumo.is_object());

    let origen = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            &json!({"codigo": "bodega", "nombre": "Bodega", "predeterminada": true}),
        )
        .await
        .expect("origen");
    let destino = ctx
        .ubicaciones
        .crear(
            EMPRESA,
            &json!({"codigo": "sucursal", "nombre": "Sucursal"}),
        )
        .await
        .expect("destino");
    let origen_id = origen["id"].as_str().expect("origen");
    let destino_id = destino["id"].as_str().expect("destino");
    ctx.ubicaciones
        .fijar_stock(EMPRESA, origen_id, item_id, &json!(8))
        .await
        .expect("stock origen");
    ctx.ubicaciones
        .transferir(
            EMPRESA,
            &json!({
                "item_id": item_id,
                "origen_id": origen_id,
                "destino_id": destino_id,
                "cantidad": 3
            }),
        )
        .await
        .expect("transferencia");
    let stock_destino = ctx
        .ubicaciones
        .stock_de_ubicacion(EMPRESA, destino_id)
        .await
        .expect("stock destino");
    assert_eq!(stock_destino[0]["stock"], 3);
}
