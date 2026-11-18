//! Pruebas PostgreSQL del dominio de catálogo (con `TEST_DATABASE_URL`).

mod common;

use common::*;
use item_service::componentes::Componentes;
use item_service::items::Items;
use item_service::productos::Productos;
use item_service::series::Series;
use item_service::servicios::Servicios;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

struct Contexto {
    _guard: tokio::sync::MutexGuard<'static, ()>,
    db: Arc<Db>,
    items: Arc<Items>,
    series: Arc<Series>,
    componentes: Arc<Componentes>,
    servicios: Arc<Servicios>,
}

async fn preparar() -> Contexto {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = db().await;
    let items = Arc::new(Items::nueva(Arc::clone(&db)));
    let productos = Arc::new(Productos::nueva(
        Arc::clone(&db),
        None,
        Some(Arc::clone(&items)),
    ));
    Contexto {
        _guard: guard,
        series: Arc::new(Series::nueva(Arc::clone(&db))),
        componentes: Arc::new(Componentes::nueva(Arc::clone(&db), Arc::clone(&items))),
        servicios: Arc::new(Servicios::con_productos(
            Arc::clone(&db),
            productos,
            Some(Arc::clone(&items)),
        )),
        db,
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
    assert!(item.get("precio").is_none());
    assert!(item.get("stock").is_none());
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

/// Regresión de ADR-0020 fase 3b: `item.items` ya no tiene `precio_cents` ni
/// `stock`; los servicios deben listarse y editarse sin esas columnas.
#[tokio::test]
async fn debe_leer_y_actualizar_servicios_sin_columnas_de_precio_stock() {
    let ctx = preparar().await;
    let id = uuid::Uuid::new_v4();
    sqlx::query(
        "INSERT INTO item.items
           (id, empresa_id, tipo, codigo, nombre, costo_cents, duration_minutes, unidad, estado)
         VALUES ($1::uuid, $2::uuid, 'servicio', 'SERV-1', 'Instalación', 5000, 60, 'Hora (h)', 'activo')",
    )
    .bind(id)
    .bind(EMPRESA)
    .execute(ctx.db.pool())
    .await
    .expect("servicio");

    let pagina = ctx
        .servicios
        .listar(&json!({"empresa_id": EMPRESA}))
        .await
        .expect("listar");
    assert_eq!(pagina["total"], 1);
    assert_eq!(pagina["items"][0]["codigo"], "SERV-1");
    assert!(pagina["items"][0].get("precio").is_none());
    assert!(pagina["items"][0].get("stock").is_none());

    let encontrado = ctx
        .servicios
        .encontrar_por_codigo("SERV-1", EMPRESA)
        .await
        .expect("codigo");
    assert!(encontrado.is_some());

    let actualizado = ctx
        .servicios
        .actualizar(
            &id.to_string(),
            &json!({"nombre": "Instalación 2", "precio": "120.00"}),
            EMPRESA,
        )
        .await
        .expect("actualizar");
    assert_eq!(actualizado["nombre"], "Instalación 2");

    ctx.servicios
        .eliminar(&id.to_string(), EMPRESA)
        .await
        .expect("eliminar");
    let pagina = ctx
        .servicios
        .listar(&json!({"empresa_id": EMPRESA}))
        .await
        .expect("listar vacio");
    assert_eq!(pagina["total"], 0);
}
