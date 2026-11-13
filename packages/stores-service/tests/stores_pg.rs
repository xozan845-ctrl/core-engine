//! Pruebas de integración PostgreSQL (solo con `TEST_DATABASE_URL`).
//!
//! Provisionan su propio esquema `stores` para correr en CI.

mod common;

use common::*;
use serde_json::json;
use std::collections::HashMap;
use stores_service::consumidor::StockConsumer;
use stores_service::tiendas::contenido::ContenidoInput;
use stores_service::tiendas::media::BackgroundInput;
use stores_service::tiendas::personalizacion::PersonalizacionInput;
use stores_service::tiendas::Tiendas;
use tokio::sync::Mutex;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

async fn preparar() -> (
    tokio::sync::MutexGuard<'static, ()>,
    std::sync::Arc<stores_service::db::Db>,
    std::sync::Arc<Tiendas>,
    std::sync::Arc<stores_service::ofertas::Ofertas>,
) {
    let url = url().expect("TEST_DATABASE_URL");
    let guard = DB_LOCK.lock().await;
    provisionar(&url).await;
    let db = db().await;
    let temas = temas(
        Some("boutique"),
        json!({"color_secundario": "#222222"}),
        Some("minimal"),
        json!({"color_primario": "#111111"}),
    );
    let storage = storage_vacio();
    let tiendas = std::sync::Arc::new(Tiendas::nueva(std::sync::Arc::clone(&db), temas, storage));
    let ofertas = std::sync::Arc::new(stores_service::ofertas::Ofertas::nueva(
        std::sync::Arc::clone(&db),
        catalogo("1000.00", Some(5), true),
    ));
    (guard, db, tiendas, ofertas)
}

fn actor(id: &str, rol: &str) -> stores_service::auth::UsuarioContexto {
    stores_service::auth::UsuarioContexto {
        user_id: id.to_string(),
        rol: rol.to_string(),
        tenant_id: Some(EMPRESA.to_string()),
        ..Default::default()
    }
}

async fn crear_tienda(tiendas: &Tiendas, nombre: &str, slug: Option<&str>) -> serde_json::Value {
    tiendas
        .crear(
            EMPRESA,
            USUARIO,
            Some(10),
            nombre,
            Some("descripcion"),
            slug,
            Some(USUARIO),
        )
        .await
        .expect("tienda")
}

#[tokio::test]
async fn debe_crear_resolver_y_redirigir_el_slug_anterior() {
    let (_guard, _db, tiendas, _ofertas) = preparar().await;
    let tienda = crear_tienda(&tiendas, "Mi Tienda", Some("mi-tienda")).await;
    assert_eq!(tienda["slug"], "mi-tienda");
    assert_eq!(tienda["participacion_porcentaje"], 10);
    assert_eq!(tienda["vendedor_id"], USUARIO);
    assert_eq!(tienda["background_overlay"], 35);
    assert_eq!(tienda["tema"]["color_primario"], "#111111");
    assert_eq!(tienda["tema"]["color_secundario"], "#222222");
    assert_eq!(tienda["plantilla"], "boutique");
    assert_eq!(
        tienda["origen_personalizacion"],
        json!({"plantilla": "empresa", "tema": "empresa"})
    );

    let resuelta = tiendas
        .resolver("mi-tienda")
        .await
        .expect("resolver")
        .expect("tienda");
    assert!(resuelta.redirigir_a.is_none());

    let actualizada = tiendas
        .actualizar_slug(USUARIO, "mi-tienda", "mi-tienda-nueva")
        .await
        .expect("slug");
    assert_eq!(actualizada["slug"], "mi-tienda-nueva");

    let anterior = tiendas
        .resolver("mi-tienda")
        .await
        .expect("resolver")
        .expect("tienda");
    assert_eq!(
        anterior.redirigir_a.as_deref(),
        Some("/api/v2/tiendas/mi-tienda-nueva")
    );

    let error = tiendas
        .actualizar_slug(OTRO_USUARIO, "mi-tienda-nueva", "otro-slug")
        .await
        .expect_err("propietario");
    assert_eq!(error.codigo, "ACCESO_DENEGADO");
}

#[tokio::test]
async fn debe_resolver_colisiones_de_slug_y_rechazar_duplicados() {
    let (_guard, _db, tiendas, _ofertas) = preparar().await;
    crear_tienda(&tiendas, "Mi Tienda", Some("mi-tienda")).await;
    let error = tiendas
        .crear(
            EMPRESA,
            USUARIO,
            Some(10),
            "Otra Tienda",
            None,
            Some("mi-tienda"),
            Some(OTRO_USUARIO),
        )
        .await
        .expect_err("duplicado");
    assert_eq!(error.status, axum::http::StatusCode::CONFLICT);

    let segunda = tiendas
        .crear(
            EMPRESA,
            USUARIO,
            Some(10),
            "Mi Tienda",
            None,
            None,
            Some(OTRO_USUARIO),
        )
        .await
        .expect("tienda");
    assert_eq!(segunda["slug"], "mi-tienda-2");
    assert_eq!(segunda["vendedor_id"], OTRO_USUARIO);
}

#[tokio::test]
async fn debe_actualizar_fondo_personalizacion_y_contenido() {
    let (_guard, _db, tiendas, _ofertas) = preparar().await;
    let tienda = crear_tienda(&tiendas, "Mi Tienda", Some("mi-tienda")).await;
    let id = tienda["id"].as_str().expect("id");
    let usuario = actor(USUARIO, "vendedor");

    let fondo = BackgroundInput::desde_json(&json!({
        "background_alt": "  Portada  ",
        "background_overlay": 40,
        "background_position": "top",
        "background_activo": true
    }));
    let error = tiendas
        .actualizar_background(&usuario, id, &fondo)
        .await
        .expect_err("activo sin objeto");
    assert_eq!(error.codigo, "BACKGROUND_INVALIDO");

    let mut objetos = HashMap::new();
    objetos.insert(uuid_objeto(), objeto(EMPRESA, "video/mp4"));
    let storage = std::sync::Arc::new(StorageFijo(objetos));
    let tiendas = Tiendas::nueva(
        std::sync::Arc::clone(&_db),
        temas(None, json!({}), None, json!({})),
        storage,
    );
    let fondo = BackgroundInput::desde_json(&json!({
        "background_objeto_id": uuid_objeto(),
        "background_alt": "  Portada  ",
        "background_overlay": 40,
        "background_position": "top",
        "background_activo": true
    }));
    let tienda = tiendas
        .actualizar_background(&usuario, id, &fondo)
        .await
        .expect("fondo");
    assert_eq!(tienda["background_media_type"], "video");
    assert_eq!(tienda["background_alt"], "Portada");
    assert_eq!(
        tienda["background_url"],
        json!(format!("/api/v2/storage/public/objetos/{}", uuid_objeto()))
    );

    let personalizacion = PersonalizacionInput::desde_json(&json!({
        "plantilla": "minimal",
        "tema": {"color_acento": "#abc"},
        "hero_titulo": " Hola ",
        "hero_cta_url": "/tienda",
        "seo_imagen_objeto_id": null
    }));
    let tienda = tiendas
        .actualizar_personalizacion(&usuario, id, &personalizacion)
        .await
        .expect("personalizacion");
    assert_eq!(tienda["plantilla"], "minimal");
    assert_eq!(tienda["tema"]["color_acento"], "#ABC");
    assert_eq!(tienda["hero_titulo"], "Hola");
    assert!(tienda["personalizacion_actualizada_en"].is_string());

    let contenido = ContenidoInput::desde_json(&json!({
        "secciones": [
            {"tipo": "ofertas", "visible": false, "orden": 1},
            {"tipo": "hero", "visible": true, "orden": 0}
        ],
        "sobre_nosotros": " Sobre ",
        "politicas": {"envio": "24h"},
        "redes": [{"red": "instagram", "url": "https://instagram.com/x"}],
        "anuncio_texto": "Promo",
        "anuncio_enlace": "/promo",
        "anuncio_activo": true
    }));
    let tienda = tiendas
        .actualizar_contenido(&usuario, id, &contenido)
        .await
        .expect("contenido");
    assert_eq!(tienda["secciones"][0]["tipo"], "hero");
    assert_eq!(tienda["politicas"]["envio"], "24h");
    assert_eq!(tienda["redes"][0]["red"], "instagram");
    assert!(tienda["contenido_actualizado_en"].is_string());
}

#[tokio::test]
async fn debe_publicar_oferta_cambiar_margen_y_sincronizar_stock() {
    let (_guard, db, tiendas, ofertas) = preparar().await;
    let tienda = crear_tienda(&tiendas, "Mi Tienda", Some("mi-tienda")).await;
    let tienda_id = tienda["id"].as_str().expect("id");

    let oferta = ofertas
        .publicar_item(USUARIO, tienda_id, ITEM, 20, "nio")
        .await
        .expect("oferta");
    assert_eq!(oferta["precio_base"], "1000.00");
    assert_eq!(oferta["precio_venta"], "1200.00");
    assert_eq!(oferta["margen"], 20);
    assert_eq!(oferta["stock"], 5);
    assert_eq!(oferta["moneda"], "NIO");
    assert_eq!(oferta["atributos"]["talla"], "M");

    let duplicada = ofertas
        .publicar_item(USUARIO, tienda_id, ITEM, 10, "NIO")
        .await
        .expect_err("duplicada");
    assert_eq!(duplicada.status, axum::http::StatusCode::CONFLICT);

    let oferta_id = oferta["id"].as_str().expect("oferta id");
    let actualizada = ofertas
        .cambiar_margen(oferta_id, USUARIO, 30)
        .await
        .expect("margen");
    assert_eq!(actualizada["precio_venta"], "1300.00");

    let historico: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM stores.historico_precios WHERE oferta_id = $1::uuid",
    )
    .bind(oferta_id)
    .fetch_one(db.pool())
    .await
    .expect("historico");
    assert_eq!(historico, 2);

    let pagina = ofertas
        .de_vendedor(USUARIO, Some("1"), Some("20"))
        .await
        .expect("pagina");
    assert_eq!(pagina["total"], 1);
    assert_eq!(pagina["paginas"], 1);
    assert_eq!(
        ofertas
            .contar_activas_por_producto(ITEM)
            .await
            .expect("cuenta"),
        1
    );

    ofertas
        .sincronizar_stock_de_oferta(EMPRESA, "sku-1", 0)
        .await
        .expect("stock 0");
    let agotada = ofertas
        .encontrar_por_id(oferta_id)
        .await
        .expect("oferta")
        .expect("existe");
    assert_eq!(agotada["stock"], 0);
    assert_eq!(agotada["estado"], "agotada");

    ofertas
        .sincronizar_stock_de_oferta(EMPRESA, "SKU-1", 7)
        .await
        .expect("stock 7");
    let activa = ofertas
        .encontrar_por_id(oferta_id)
        .await
        .expect("oferta")
        .expect("existe");
    assert_eq!(activa["stock"], 7);
    assert_eq!(activa["estado"], "activa");
}

#[tokio::test]
async fn debe_consumir_stock_updated_una_sola_vez() {
    let (_guard, db, tiendas, ofertas) = preparar().await;
    let tienda = crear_tienda(&tiendas, "Mi Tienda", Some("mi-tienda")).await;
    let oferta = ofertas
        .publicar_item(USUARIO, tienda["id"].as_str().expect("id"), ITEM, 20, "NIO")
        .await
        .expect("oferta");
    let oferta_id = oferta["id"].as_str().expect("oferta id");
    let consumidor =
        StockConsumer::nuevo(std::sync::Arc::clone(&db), std::sync::Arc::clone(&ofertas));
    let evento = json!({
        "event_id": "130c07c6-7e10-4f91-9c13-e17ca6be6eea",
        "tipo": "stock.updated",
        "data": {"items": [{"empresa_id": EMPRESA, "sku": "SKU-1", "stock_restante": 3}]}
    });
    consumidor.manejar(evento.clone()).await.expect("consumo");

    let despues = ofertas
        .encontrar_por_id(oferta_id)
        .await
        .expect("oferta")
        .expect("existe");
    assert_eq!(despues["stock"], 3);

    sqlx::query("UPDATE stores.ofertas SET stock = 9 WHERE id = $1::uuid")
        .bind(oferta_id)
        .execute(db.pool())
        .await
        .expect("stock manual");
    consumidor.manejar(evento).await.expect("reintento");
    let sin_cambio = ofertas
        .encontrar_por_id(oferta_id)
        .await
        .expect("oferta")
        .expect("existe");
    assert_eq!(sin_cambio["stock"], 9);

    let otro = json!({
        "event_id": "230c07c6-7e10-4f91-9c13-e17ca6be6eea",
        "tipo": "stock.updated",
        "data": {"items": [{"empresa_id": EMPRESA, "sku": "SKU-1", "stock_restante": 0}]}
    });
    consumidor.manejar(otro).await.expect("segundo evento");
    let agotada = ofertas
        .encontrar_por_id(oferta_id)
        .await
        .expect("oferta")
        .expect("existe");
    assert_eq!(agotada["estado"], "agotada");
}

#[tokio::test]
async fn debe_asignar_y_desasignar_vendedor_y_listar_por_ids() {
    let (_guard, _db, tiendas, _ofertas) = preparar().await;
    let tienda = tiendas
        .crear(
            EMPRESA,
            USUARIO,
            None,
            "Tienda Libre",
            None,
            Some("libre"),
            None,
        )
        .await
        .expect("tienda");
    let id = tienda["id"].as_str().expect("id");
    assert!(tienda["vendedor_id"].is_null());

    tiendas
        .asignar_vendedor(id, EMPRESA, OTRO_USUARIO, 50)
        .await
        .expect("asignar");
    let asignada = tiendas.de_vendedor(OTRO_USUARIO).await.expect("vendedor");
    assert_eq!(asignada.expect("tienda")["participacion_porcentaje"], 50);
    let error = tiendas
        .asignar_vendedor(id, EMPRESA, USUARIO, 10)
        .await
        .expect_err("ya asignada");
    assert_eq!(error.codigo, "TIENDA_YA_ASIGNADA");

    let lote = tiendas.por_ids(&[id.to_string()]).await.expect("lote");
    assert_eq!(lote.len(), 1);
    let empresa = tiendas.de_empresa(EMPRESA).await.expect("empresa");
    assert_eq!(empresa.len(), 1);

    tiendas
        .desasignar_vendedor(id, OTRO_USUARIO)
        .await
        .expect("desasignar");
    assert!(tiendas
        .de_vendedor(OTRO_USUARIO)
        .await
        .expect("vendedor")
        .is_none());
}
