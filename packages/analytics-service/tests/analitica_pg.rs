//! Pruebas de integración PostgreSQL del servicio de analítica.

use analytics_service::clients::{ItemOrden, OrdenInterna, Ordenes};
use analytics_service::consumidor::AnalyticsConsumer;
use analytics_service::db::Db;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

async fn preparar() -> Option<(Arc<Db>, sqlx::PgPool)> {
    let url = url()?;
    let pool = sqlx::PgPool::connect(&url).await.expect("postgres");
    sqlx::raw_sql(include_str!("sql/analytics.sql"))
        .execute(&pool)
        .await
        .expect("migracion analytics");
    sqlx::raw_sql(
        "TRUNCATE analytics.hechos_venta, analytics.comisiones,
                  analytics.eventos_procesados, analytics.reporte_ejecuciones,
                  analytics.reportes_programados, identity.empresas, identity.negocios
         CASCADE",
    )
    .execute(&pool)
    .await
    .expect("limpieza analytics");
    let db = Arc::new(Db::conectar(&url).await.expect("pool"));
    Some((db, pool))
}

struct OrdenFija {
    orden: OrdenInterna,
}

#[async_trait]
impl Ordenes for OrdenFija {
    async fn obtener(&self, _order_id: &str) -> Option<OrdenInterna> {
        Some(self.orden.clone())
    }
}

fn item(empresa: Uuid, negocio: Uuid, tienda: Uuid, oferta: &str, cantidad: i64) -> ItemOrden {
    ItemOrden {
        oferta_id: oferta.to_string(),
        vendedor_id: Some(Uuid::new_v4().to_string()),
        empresa_id: Some(empresa.to_string()),
        negocio_id: Some(negocio.to_string()),
        tienda_id: Some(tienda.to_string()),
        participacion_porcentaje: Some(60),
        precio_unitario_cents: 1000,
        cantidad,
    }
}

#[tokio::test]
async fn debe_calcular_negocio_empresa_plataforma_y_cohortes() {
    let _guard = DB_LOCK.lock().await;
    let Some((db, pool)) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integracion");
        return;
    };
    let empresa = Uuid::new_v4();
    let otra_empresa = Uuid::new_v4();
    let negocio_a = Uuid::new_v4();
    let negocio_b = Uuid::new_v4();
    let comprador_a = Uuid::new_v4();
    let comprador_b = Uuid::new_v4();
    sqlx::query("INSERT INTO identity.empresas (id, nombre) VALUES ($1, 'Centro'), ($2, 'Norte')")
        .bind(empresa)
        .bind(otra_empresa)
        .execute(&pool)
        .await
        .expect("empresas");
    sqlx::query("INSERT INTO identity.negocios (id, nombre) VALUES ($1, 'Centro'), ($2, 'Norte')")
        .bind(negocio_a)
        .bind(negocio_b)
        .execute(&pool)
        .await
        .expect("negocios");

    let hechos = [
        (
            Uuid::new_v4(),
            "of-1",
            empresa,
            negocio_a,
            comprador_a,
            2,
            "2026-05-10T10:00:00Z",
        ),
        (
            Uuid::new_v4(),
            "of-1",
            empresa,
            negocio_b,
            comprador_b,
            1,
            "2026-06-02T10:00:00Z",
        ),
        (
            Uuid::new_v4(),
            "of-2",
            empresa,
            negocio_a,
            comprador_a,
            1,
            "2026-06-15T10:00:00Z",
        ),
        (
            Uuid::new_v4(),
            "of-1",
            otra_empresa,
            Uuid::new_v4(),
            Uuid::new_v4(),
            1,
            "2026-06-20T10:00:00Z",
        ),
    ];
    for (order_id, linea, emp, neg, comp, cantidad, fecha) in hechos {
        sqlx::query(
            "INSERT INTO analytics.hechos_venta
               (order_id, linea_clave, empresa_id, negocio_id, tienda_id, vendedor_id,
                comprador_id, cantidad, precio_unitario_cents, venta_cents, orden_creada_en)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1000, $9, $10::timestamptz)",
        )
        .bind(order_id)
        .bind(linea)
        .bind(emp)
        .bind(neg)
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .bind(comp)
        .bind(cantidad)
        .bind(cantidad * 1000)
        .bind(fecha)
        .execute(&pool)
        .await
        .expect("hecho");
    }
    sqlx::query(
        "INSERT INTO analytics.comisiones
           (order_id, linea_clave, empresa_id, negocio_id, tienda_id, vendedor_id,
            comision_cents, monto_vendedor_cents, monto_empresario_cents)
         VALUES ($1, 'of-2', $2, $3, $4, $5, 170, 900, 3830)",
    )
    .bind(hechos[2].0)
    .bind(empresa)
    .bind(negocio_a)
    .bind(Uuid::new_v4())
    .bind(Uuid::new_v4())
    .execute(&pool)
    .await
    .expect("comision");

    let empresa_txt = empresa.to_string();
    let resumen = analytics_service::analitica::resumen_negocios(
        &db,
        &empresa_txt,
        None,
        Some("2026-01-01"),
        Some("2026-12-31T23:59:59Z"),
    )
    .await
    .expect("resumen");
    assert_eq!(resumen["totales"]["ordenes"], 3);
    assert_eq!(resumen["totales"]["ventas_cents"], 4000);
    assert_eq!(resumen["totales"]["comisiones_cents"], 170);
    assert_eq!(resumen["negocios"].as_array().expect("negocios").len(), 2);

    let cohortes = analytics_service::analitica::cohortes_negocios(
        &db,
        &empresa_txt,
        None,
        Some("2026-01-01"),
        Some("2026-12-31T23:59:59Z"),
    )
    .await
    .expect("cohortes");
    assert_eq!(cohortes["totales"]["cohortes"], 2);
    assert_eq!(cohortes["totales"]["clientes"], 2);
    assert_eq!(cohortes["totales"]["retencion_mes_1_pct"], 50.0);

    let plataforma = analytics_service::analitica::resumen_plataforma(
        &db,
        Some("2026-01-01"),
        Some("2026-12-31T23:59:59Z"),
    )
    .await
    .expect("plataforma");
    assert_eq!(plataforma["totales"]["empresas"], 2);
    assert_eq!(plataforma["totales"]["ventas_cents"], 5000);
    assert_eq!(
        plataforma["empresas"].as_array().expect("empresas").len(),
        2
    );
}

#[tokio::test]
async fn debe_proyectar_eventos_de_orden_y_comision_una_sola_vez() {
    let _guard = DB_LOCK.lock().await;
    let Some((db, pool)) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integracion");
        return;
    };
    let empresa = Uuid::new_v4();
    let negocio = Uuid::new_v4();
    let tienda = Uuid::new_v4();
    let comprador = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    let oferta = Uuid::new_v4();
    let orden = OrdenInterna {
        comprador_id: Some(comprador.to_string()),
        creado_en: Some("2026-06-01T10:00:00Z".to_string()),
        items: vec![item(empresa, negocio, tienda, &oferta.to_string(), 2)],
    };
    let consumidor = AnalyticsConsumer::nuevo(Arc::clone(&db), Arc::new(OrdenFija { orden }));
    let evento = json!({
        "event_id": Uuid::new_v4().to_string(),
        "tipo": "order.completado",
        "data": { "order_id": order_id.to_string() }
    });
    consumidor.manejar(evento.clone()).await.expect("primera");
    consumidor.manejar(evento).await.expect("idempotente");

    let hechos: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM analytics.hechos_venta WHERE order_id = $1")
            .bind(order_id)
            .fetch_one(&pool)
            .await
            .expect("hechos");
    assert_eq!(hechos.0, 1);

    let comision = json!({
        "event_id": Uuid::new_v4().to_string(),
        "tipo": "comision.acreditada",
        "data": {
            "order_id": order_id.to_string(),
            "linea_clave": oferta.to_string(),
            "empresa_id": empresa.to_string(),
            "negocio_id": negocio.to_string(),
            "tienda_id": tienda.to_string(),
            "vendedor_id": Uuid::new_v4().to_string(),
            "monto_cents": 120,
            "monto_vendedor_cents": 1080,
            "monto_empresario_cents": 800,
            "participacion_porcentaje": 60
        }
    });
    consumidor
        .manejar(comision.clone())
        .await
        .expect("comision");
    consumidor
        .manejar(comision)
        .await
        .expect("comision idempotente");

    let comisiones: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM analytics.comisiones WHERE order_id = $1")
            .bind(order_id)
            .fetch_one(&pool)
            .await
            .expect("comisiones");
    assert_eq!(comisiones.0, 1);
}

#[tokio::test]
async fn debe_guardar_y_ejecutar_reportes() {
    use chrono::{Duration, Utc};
    let _guard = DB_LOCK.lock().await;
    let Some((db, _pool)) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integracion");
        return;
    };
    let empresa = Uuid::new_v4().to_string();
    let reporte = analytics_service::reportes::crear(
        &db,
        &empresa,
        &Uuid::new_v4().to_string(),
        analytics_service::reportes::NuevoReporte {
            nombre: "Cohortes".to_string(),
            tipo: "cohortes".to_string(),
            formato: "csv".to_string(),
            frecuencia: "diaria".to_string(),
            destino: "ops@test.dev".to_string(),
            proxima_ejecucion: (Utc::now() - Duration::minutes(1)).to_rfc3339(),
            negocio_id: None,
        },
    )
    .await
    .expect("reporte");
    let id = reporte["id"].as_str().expect("id").to_string();
    let ejecucion = analytics_service::reportes::ejecutar(&db, &empresa, &id)
        .await
        .expect("ejecutar");
    assert_eq!(ejecucion["estado"], "ok");
    assert!(ejecucion["contenido"]
        .as_str()
        .expect("contenido")
        .starts_with("cohorte,indice"));
    assert_eq!(
        analytics_service::reportes::ejecuciones(&db, &empresa, &id)
            .await
            .expect("historial")
            .len(),
        1
    );
}
