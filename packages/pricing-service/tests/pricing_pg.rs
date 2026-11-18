mod common;

use common::*;
use pricing_service::cupones::Cupones;
use pricing_service::precios::Precios;
use pricing_service::promociones::Promociones;
use pricing_service::tarifas::Tarifas;
use serde_json::json;
use std::sync::Arc;

struct Contexto {
    _guard: DbLock,
    precios: Arc<Precios>,
    tarifas: Arc<Tarifas>,
    promociones: Arc<Promociones>,
    cupones: Arc<Cupones>,
    db: Arc<Db>,
}

async fn preparar() -> Contexto {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = provisionar(&url).await;
    let db = db().await;
    Contexto {
        precios: Arc::new(Precios::nueva(Arc::clone(&db))),
        tarifas: Arc::new(Tarifas::nueva(Arc::clone(&db))),
        promociones: Arc::new(Promociones::nueva(Arc::clone(&db))),
        cupones: Arc::new(Cupones::nueva(Arc::clone(&db))),
        db,
        _guard: guard,
    }
}

#[tokio::test]
async fn debe_resolver_listas_escalas_y_precio_base() {
    let ctx = preparar().await;
    crear_item(&ctx.db, "SKU-PRECIO", 100_000).await;
    let lista = ctx
        .precios
        .crear_lista(
            EMPRESA,
            &json!({
                "codigo": "mayorista", "nombre": "Mayorista", "moneda": "NIO"
            }),
        )
        .await
        .expect("lista");
    let lista_id = lista["id"].as_str().expect("id");
    ctx.precios
        .crear_precio(
            EMPRESA,
            Some(lista_id),
            &json!({
                "item_id": ITEM, "desde_cantidad": 1, "precio_cents": 90000
            }),
        )
        .await
        .expect("precio lista");
    ctx.precios
        .crear_escala(
            EMPRESA,
            ITEM,
            &json!({
                "desde_cantidad": 10, "hasta_cantidad": null, "precio_cents": 80000
            }),
        )
        .await
        .expect("escala");

    let por_lista = ctx
        .precios
        .precio_de(
            EMPRESA,
            ITEM,
            &json!("1"),
            &json!({"lista_codigo": "mayorista"}),
        )
        .await
        .expect("lista");
    assert_eq!(por_lista["origen"], "lista");
    assert_eq!(por_lista["precio_unitario_cents"], 90000);

    let por_escala = ctx
        .precios
        .precio_de(EMPRESA, ITEM, &json!("12"), &json!({}))
        .await
        .expect("escala");
    assert_eq!(por_escala["origen"], "escala");
    assert_eq!(por_escala["precio_total_cents"], 960000);

    let base = ctx
        .precios
        .precio_de(EMPRESA, ITEM, &json!("1"), &json!({}))
        .await
        .expect("base");
    assert_eq!(base["origen"], "base");
    assert_eq!(base["precio_unitario_cents"], 100000);
}

#[tokio::test]
async fn debe_resolver_precios_en_lote() {
    let ctx = preparar().await;
    crear_item(&ctx.db, "SKU-LOTE", 100_000).await;
    let lote = ctx
        .precios
        .precios_lote(&json!({
            "empresa_id": EMPRESA,
            "lineas": [{"clave": "l1", "item_id": ITEM, "cantidad": 2}]
        }))
        .await
        .expect("lote");
    assert_eq!(lote["precios"][0]["clave"], "l1");
    assert_eq!(lote["precios"][0]["precio_total_cents"], 200000);
}

#[tokio::test]
async fn debe_devolver_la_promocion_vigente_en_el_lote_de_precios() {
    let ctx = preparar().await;
    crear_item(&ctx.db, "SKU-PROMO", 100_000).await;
    let promo = ctx
        .promociones
        .crear(
            EMPRESA,
            &json!({
                "nombre": "25% de descuento",
                "tipo": "descuento_porcentaje",
                "item_id": ITEM,
                "descuento_porcentaje": 25,
                "fecha_desde": "2020-01-01",
                "fecha_hasta": "2030-12-31"
            }),
        )
        .await
        .expect("promocion");
    assert_eq!(promo["tipo"], "descuento_porcentaje");

    let lote = ctx
        .precios
        .precios_lote(&json!({
            "empresa_id": EMPRESA,
            "lineas": [{
                "clave": "l1",
                "item_id": ITEM,
                "sku": "SKU-PROMO",
                "cantidad": 2
            }]
        }))
        .await
        .expect("lote");
    assert_eq!(
        lote["precios"][0]["promocion"]["tipo"],
        "descuento_porcentaje"
    );
    assert_eq!(lote["precios"][0]["promocion"]["descuento_porcentaje"], 25);
}

#[tokio::test]
async fn debe_cotizar_tarifas_por_rango() {
    let ctx = preparar().await;
    crear_item(&ctx.db, "SKU-TARIFA", 100_000).await;
    ctx.tarifas
        .crear(
            EMPRESA,
            ITEM,
            &json!({
                "desde": "2026-12-20", "hasta": "2026-12-22", "precio_cents": 20000
            }),
        )
        .await
        .expect("tarifa");
    let cotizacion = ctx
        .tarifas
        .cotizacion(EMPRESA, ITEM, &json!("2026-12-19"), &json!("2026-12-23"))
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
async fn debe_gestionar_promociones_y_resolver_el_item() {
    let ctx = preparar().await;
    crear_item(&ctx.db, "SKU-PROMO", 100_000).await;
    let promo = ctx
        .promociones
        .crear(
            EMPRESA,
            &json!({
                "nombre": "Promo 2x1",
                "tipo": "2x1",
                "sku": "SKU-PROMO",
                "fecha_desde": "2026-11-01",
                "fecha_hasta": "2026-11-30"
            }),
        )
        .await
        .expect("promo");
    assert_eq!(promo["item_id"], ITEM);
    assert_eq!(promo["sku"], "SKU-PROMO");
    assert_eq!(promo["cantidad_requerida"], 2);

    let lista = ctx
        .promociones
        .listar(EMPRESA, &json!({"pagina": 1, "limite": 20}))
        .await
        .expect("listar");
    assert_eq!(lista["total"], 1);
    let detalle = ctx
        .promociones
        .obtener(promo["id"].as_str().unwrap(), EMPRESA)
        .await
        .expect("detalle");
    assert_eq!(detalle.expect("promo")["nombre"], "Promo 2x1");
    ctx.promociones
        .eliminar(promo["id"].as_str().unwrap(), EMPRESA)
        .await
        .expect("eliminar");
    assert!(ctx
        .promociones
        .obtener(promo["id"].as_str().unwrap(), EMPRESA)
        .await
        .expect("detalle")
        .is_none());
}

#[tokio::test]
async fn debe_validar_y_canjear_un_cupon_una_sola_vez_por_orden() {
    let ctx = preparar().await;
    let cupon = ctx
        .cupones
        .crear(
            EMPRESA,
            &json!({
                "codigo": "verano-25",
                "nombre": "Verano 25",
                "tipo": "porcentaje",
                "valor": 25,
                "minimo_compra_cents": 10_000,
                "usos_maximos": 2,
                "fecha_desde": "2020-01-01",
                "fecha_hasta": "2030-12-31"
            }),
        )
        .await
        .expect("cupon");
    assert_eq!(cupon["codigo"], "VERANO-25");

    let validado = ctx
        .cupones
        .validar(EMPRESA, "verano-25", 20_000)
        .await
        .expect("validar");
    assert_eq!(validado.descuento_cents, 5_000);

    let order_id = uuid::Uuid::new_v4().to_string();
    let canje = ctx
        .cupones
        .canjear(EMPRESA, "VERANO-25", &order_id, 20_000)
        .await
        .expect("canjear");
    assert_eq!(canje.descuento_cents, 5_000);
    let repetido = ctx
        .cupones
        .canjear(EMPRESA, "VERANO-25", &order_id, 20_000)
        .await
        .expect("idempotente");
    assert_eq!(repetido.descuento_cents, 5_000);
    let usos: i32 =
        sqlx::query_scalar("SELECT usos_actuales FROM pricing.cupones WHERE id = $1::uuid")
            .bind(uuid::Uuid::parse_str(cupon["id"].as_str().expect("id")).expect("uuid"))
            .fetch_one(ctx.db.pool())
            .await
            .expect("usos");
    assert_eq!(usos, 1);

    let minimo = ctx
        .cupones
        .validar(EMPRESA, "VERANO-25", 1_000)
        .await
        .expect_err("minimo");
    assert_eq!(minimo.codigo, "CUPON_MINIMO");
}

#[tokio::test]
async fn debe_gestionar_cupones_y_respetar_usos_y_liberacion() {
    let ctx = preparar().await;
    let cupon = ctx
        .cupones
        .crear(
            EMPRESA,
            &json!({
                "codigo": "FIJO-100",
                "nombre": "Cien pesos",
                "tipo": "monto_fijo",
                "valor": 1_000,
                "minimo_compra_cents": 5_000,
                "usos_maximos": 1,
                "fecha_desde": "2020-01-01",
                "fecha_hasta": "2030-12-31"
            }),
        )
        .await
        .expect("cupon");
    let id = cupon["id"].as_str().expect("id");

    let listado = ctx
        .cupones
        .listar(EMPRESA, &json!({ "pagina": 1, "limite": 10 }))
        .await
        .expect("listar");
    assert_eq!(listado["total"], 1);
    let detalle = ctx
        .cupones
        .obtener(id, EMPRESA)
        .await
        .expect("detalle")
        .expect("existe");
    assert_eq!(detalle["codigo"], "FIJO-100");

    let actualizado = ctx
        .cupones
        .actualizar(id, EMPRESA, &json!({ "nombre": "Cien pesos v2" }))
        .await
        .expect("actualizar");
    assert_eq!(actualizado["nombre"], "Cien pesos v2");

    let minimo = ctx
        .cupones
        .validar(EMPRESA, "FIJO-100", 4_000)
        .await
        .expect_err("minimo");
    assert_eq!(minimo.codigo, "CUPON_MINIMO");

    let order_1 = uuid::Uuid::new_v4().to_string();
    let order_2 = uuid::Uuid::new_v4().to_string();
    let primero = ctx
        .cupones
        .canjear(EMPRESA, "FIJO-100", &order_1, 10_000)
        .await
        .expect("canje");
    assert_eq!(primero.descuento_cents, 1_000);
    let agotado = ctx
        .cupones
        .canjear(EMPRESA, "FIJO-100", &order_2, 10_000)
        .await
        .expect_err("agotado");
    assert_eq!(agotado.codigo, "CUPON_AGOTADO");

    ctx.cupones
        .liberar(EMPRESA, "FIJO-100", &order_1)
        .await
        .expect("liberar");
    ctx.cupones
        .canjear(EMPRESA, "FIJO-100", &order_2, 10_000)
        .await
        .expect("canje tras liberar");

    ctx.cupones.eliminar(id, EMPRESA).await.expect("eliminar");
    assert!(ctx
        .cupones
        .obtener(id, EMPRESA)
        .await
        .expect("detalle")
        .is_none());
}
