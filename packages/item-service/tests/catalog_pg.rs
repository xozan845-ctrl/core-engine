//! Pruebas PostgreSQL del dominio de catálogo (con `TEST_DATABASE_URL`).

mod common;

use common::*;
use item_service::componentes::Componentes;
use item_service::items::Items;
use item_service::series::Series;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

struct Contexto {
    _guard: tokio::sync::MutexGuard<'static, ()>,
    items: Arc<Items>,
    series: Arc<Series>,
    componentes: Arc<Componentes>,
}

async fn preparar() -> Contexto {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = db().await;
    let items = Arc::new(Items::nueva(Arc::clone(&db)));
    Contexto {
        _guard: guard,
        series: Arc::new(Series::nueva(Arc::clone(&db))),
        componentes: Arc::new(Componentes::nueva(Arc::clone(&db), Arc::clone(&items))),
        items,
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
