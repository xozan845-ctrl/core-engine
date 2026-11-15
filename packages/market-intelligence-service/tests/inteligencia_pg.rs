//! Pruebas de integración PostgreSQL/PostGIS (solo con `TEST_DATABASE_URL`).
//! Provisionan su propio esquema `intelligence` desde las migraciones reales.

use market_intelligence_service::agregador::AgregadorPort;
use market_intelligence_service::calidad::CalidadDatos;
use market_intelligence_service::consumidor::InteligenciaConsumer;
use market_intelligence_service::db::Db;
use market_intelligence_service::inteligencia::{Filtros, Inteligencia, VentaGeolocalizada};
use market_intelligence_service::metrics::Metrics;
use market_intelligence_service::tareas::{computar_features, refrescar_vistas};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

fn migracion(texto: &'static str) -> &'static str {
    texto.trim_start_matches('\u{feff}')
}

async fn provisionar(url: &str) {
    let pool = sqlx::PgPool::connect(url).await.expect("postgres");
    sqlx::raw_sql("DROP SCHEMA IF EXISTS intelligence CASCADE;")
        .execute(&pool)
        .await
        .expect("reset intelligence");
    for sql in [
        migracion(include_str!("../../../infra/db/init/03_intelligence.sql")),
        migracion(include_str!(
            "../../../infra/db/init/04_intelligence_postgis.sql"
        )),
        migracion(include_str!(
            "../../../infra/db/init/05_materialized_views.sql"
        )),
        migracion(include_str!("../../../infra/db/init/06_feature_store.sql")),
        migracion(include_str!("../../../infra/db/init/07_data_quality.sql")),
    ] {
        sqlx::raw_sql(sql)
            .execute(&pool)
            .await
            .expect("migracion intelligence");
    }
    // Equivalente a 03a sin el backfill desde catalog (no existe en la BD de test).
    sqlx::raw_sql(
        "ALTER TABLE intelligence.metricas_producto ADD COLUMN empresa_id uuid;
         ALTER TABLE intelligence.metricas_producto DROP CONSTRAINT metricas_producto_pkey;
         ALTER TABLE intelligence.metricas_producto ALTER COLUMN empresa_id SET NOT NULL;
         ALTER TABLE intelligence.metricas_producto
           ADD CONSTRAINT metricas_producto_pkey PRIMARY KEY (empresa_id, sku);",
    )
    .execute(&pool)
    .await
    .expect("pk metricas_producto");
    pool.close().await;
}

fn venta(order_id: &str, vendedor_id: &str, monto: i64) -> VentaGeolocalizada {
    VentaGeolocalizada {
        order_id: order_id.to_string(),
        vendedor_id: vendedor_id.to_string(),
        skus: vec!["SKU-1".to_string(), "SKU-2".to_string()],
        monto_cents: monto,
        lat: Some(12.1),
        lng: Some(-86.2),
        precision: Some(5.0),
        velocidad: Some(10.0),
        rumbo: Some(90.0),
        tipo_actividad: Some("venta".to_string()),
        resultado_visita: Some("visitado".to_string()),
        distancia_cliente_metros: Some(250.0),
        rango_edad: Some("25-34".to_string()),
        genero: Some("M".to_string()),
    }
}

async fn contar(db: &Db, sql: &str) -> i64 {
    sqlx::query_scalar(sql)
        .fetch_one(db.pool())
        .await
        .expect("conteo")
}

#[tokio::test]
async fn debe_agregar_venta_geo_de_forma_idempotente_y_consultar_metricas() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;

    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let inteligencia = Inteligencia::nueva(db.clone());
    let order_id = Uuid::new_v4().to_string();
    let vendedor_id = Uuid::new_v4().to_string();
    let event_id = Uuid::new_v4().to_string();

    let data = venta(&order_id, &vendedor_id, 1_000);
    inteligencia
        .procesar_venta_geolocalizada(&data, Some(&event_id))
        .await
        .expect("primera venta");
    inteligencia
        .procesar_venta_geolocalizada(&data, Some(&event_id))
        .await
        .expect("replay idempotente");

    assert_eq!(
        contar(&db, "SELECT COUNT(*) FROM intelligence.hechos_venta").await,
        2
    );
    assert_eq!(
        contar(&db, "SELECT COUNT(*) FROM intelligence.puntos_calor").await,
        1
    );
    assert_eq!(
        contar(&db, "SELECT COUNT(*) FROM intelligence.eventos_procesados").await,
        1
    );
    assert_eq!(
        contar(
            &db,
            "SELECT COUNT(*) FROM intelligence.metricas_producto WHERE empresa_id = '00000000-0000-0000-0000-000000000001'"
        )
        .await,
        2
    );

    let mapa = inteligencia
        .obtener_mapa_calor(&Filtros::default())
        .await
        .expect("mapa");
    assert_eq!(mapa["total_puntos"], 1);
    assert_eq!(mapa["puntos"][0]["tipo"], "venta");

    let rendimiento = inteligencia
        .obtener_rendimiento(&Filtros::default())
        .await
        .expect("rendimiento");
    assert_eq!(rendimiento["data"][0]["total_ventas"], 1);
    // node-postgres devuelve bigint como string.
    assert_eq!(rendimiento["data"][0]["total_monto_cents"], "1000");
    assert_eq!(rendimiento["data"][0]["ticket_promedio_cents"], "1000");
    assert!(rendimiento["next_cursor"].is_null());

    let demanda = inteligencia
        .obtener_demanda(&Filtros::default())
        .await
        .expect("demanda");
    assert_eq!(demanda["data"].as_array().expect("data").len(), 2);
    assert_eq!(demanda["data"][0]["total_monto_cents"], "500");

    let cobertura = inteligencia
        .obtener_cobertura(&Filtros::default())
        .await
        .expect("cobertura");
    assert_eq!(cobertura[0]["total_monto_cents"], "1000");

    refrescar_vistas(&db).await.expect("vistas");
    let resumen = inteligencia
        .obtener_resumen(&Filtros::default())
        .await
        .expect("resumen");
    assert_eq!(resumen["total_pedidos"], "1");
    assert_eq!(resumen["ingresos_totales_cents"], "1000");
    assert_eq!(resumen["vendedores_activos"], "1");

    let tendencias = inteligencia
        .obtener_tendencias(&Filtros::default())
        .await
        .expect("tendencias");
    let filas = tendencias.as_array().expect("tendencias");
    assert_eq!(filas.len(), 2);
    assert_eq!(filas[0]["total_ventas"], "1");
    assert!(filas[0]["dia_semana"].is_string());
}

#[tokio::test]
async fn debe_procesar_comision_y_stock_actualizado() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;

    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let inteligencia = Inteligencia::nueva(db.clone());
    let vendedor_id = Uuid::new_v4().to_string();
    let order_id = Uuid::new_v4().to_string();
    inteligencia
        .procesar_venta_geolocalizada(&venta(&order_id, &vendedor_id, 1_000), None)
        .await
        .expect("venta");

    inteligencia
        .procesar_orden_completada(&order_id, &vendedor_id)
        .await
        .expect("comision");
    let (pedidos, tasa): (i32, String) = sqlx::query_as(
        "SELECT pedidos_entregados, tasa_efectividad::text
         FROM intelligence.rendimiento_vendedor WHERE vendedor_id = $1::uuid",
    )
    .bind(&vendedor_id)
    .fetch_one(db.pool())
    .await
    .expect("rendimiento");
    assert_eq!(pedidos, 1);
    assert_eq!(tasa, "100.00");

    let empresa_id = Uuid::new_v4().to_string();
    inteligencia
        .procesar_stock_actualizado(&empresa_id, "SKU-9", 7, None)
        .await
        .expect("stock");
    let (stock, nombre): (i32, String) = sqlx::query_as(
        "SELECT stock_actual, nombre FROM intelligence.metricas_producto
         WHERE empresa_id = $1::uuid AND sku = 'SKU-9'",
    )
    .bind(&empresa_id)
    .fetch_one(db.pool())
    .await
    .expect("metricas");
    assert_eq!(stock, 7);
    assert_eq!(nombre, "");
}

#[tokio::test]
async fn debe_computar_features_y_responder_forecast_score_y_calidad() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;

    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let inteligencia = Inteligencia::nueva(db.clone());
    let vendedor_id = Uuid::new_v4().to_string();
    for indice in 0..5 {
        let order_id = Uuid::new_v4().to_string();
        inteligencia
            .procesar_venta_geolocalizada(&venta(&order_id, &vendedor_id, 1_000), None)
            .await
            .unwrap_or_else(|error| panic!("venta {indice}: {}", error.mensaje));
    }
    inteligencia
        .procesar_orden_completada("00000000-0000-4000-8000-000000000001", &vendedor_id)
        .await
        .expect("comision");
    computar_features(&db).await.expect("features");

    let forecast = inteligencia
        .obtener_forecast(Some("SKU-1"), 7)
        .await
        .expect("forecast");
    assert_eq!(forecast["dias_proyectados"], 7);
    assert_eq!(forecast["metodologia"], "media_movil_ponderada_7d");
    assert!(forecast["ventas_proyectadas"].is_number());

    let score = inteligencia
        .obtener_score(&vendedor_id)
        .await
        .expect("score")
        .expect("con datos");
    assert!(score["churn_risk_nivel"].is_string());
    assert!(score["tasa_efectividad"].is_number());

    let calidad = CalidadDatos::nueva(
        Arc::new(Metrics::nueva("market-intelligence-service")),
        db.clone(),
    );
    calidad
        .registrar_invalido(
            Some("evento-invalido"),
            "venta.geolocalizada",
            &["monto_cents debe ser positivo".to_string()],
            &json!({"order_id": "o-1"}),
        )
        .await;
    let dashboard = inteligencia.obtener_calidad(24).await.expect("calidad");
    assert_eq!(dashboard["eventos_procesados"], 5);
    assert_eq!(dashboard["eventos_invalidos"], 1);
    assert_eq!(
        dashboard["errores_frecuentes"][0]["event_tipo"],
        "venta.geolocalizada"
    );
}

#[tokio::test]
async fn debe_rechazar_una_venta_invalida_en_el_consumidor_y_auditarla() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;

    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let inteligencia = Inteligencia::nueva(db.clone());
    let agregador: Arc<dyn AgregadorPort> = Arc::new(inteligencia.clone());
    let metrics = Arc::new(Metrics::nueva("market-intelligence-service"));
    let calidad = Arc::new(CalidadDatos::nueva(metrics, db.clone()));
    let consumidor = InteligenciaConsumer::nuevo(db.clone(), agregador, calidad);

    let event_id = Uuid::new_v4().to_string();
    consumidor
        .manejar_venta(json!({
            "event_id": event_id,
            "tipo": "venta.geolocalizada",
            "data": {
                "order_id": Uuid::new_v4().to_string(),
                "vendedor_id": Uuid::new_v4().to_string(),
                "skus": ["SKU-1"],
                "monto_cents": 0
            }
        }))
        .await
        .expect("consumir invalido");

    assert_eq!(
        contar(&db, "SELECT COUNT(*) FROM intelligence.invalid_events").await,
        1
    );
    assert_eq!(
        contar(&db, "SELECT COUNT(*) FROM intelligence.eventos_procesados").await,
        1
    );
    assert_eq!(
        contar(&db, "SELECT COUNT(*) FROM intelligence.hechos_venta").await,
        0
    );

    let event_id = Uuid::new_v4().to_string();
    consumidor
        .manejar_venta(json!({
            "event_id": event_id,
            "tipo": "venta.geolocalizada",
            "data": {
                "order_id": Uuid::new_v4().to_string(),
                "vendedor_id": Uuid::new_v4().to_string(),
                "skus": ["SKU-1"],
                "monto_cents": 1000
            }
        }))
        .await
        .expect("consumir valido");
    assert_eq!(
        contar(&db, "SELECT COUNT(*) FROM intelligence.hechos_venta").await,
        1
    );
}
