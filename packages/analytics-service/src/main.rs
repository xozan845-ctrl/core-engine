//! Arranque de `analytics-service`: PostgreSQL, RabbitMQ, HTTP y scheduler.

use analytics_service::auth::{Capacidades, CapacidadesHttp};
use analytics_service::clients::{Ordenes, OrdenesHttp};
use analytics_service::config::Config;
use analytics_service::consumidor::{
    handler_consumidor, AnalyticsConsumer, COLA_COMISIONES, COLA_ORDERS,
};
use analytics_service::db::Db;
use analytics_service::http::build_router;
use analytics_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use analytics_service::metrics::Metrics;
use analytics_service::rabbit::{ColaConfig, Rabbit};
use analytics_service::state::AppState;
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = config.validar_arranque() {
        eprintln!("{error}");
        std::process::exit(1);
    }

    let contexto = ContextoCorrelacion {
        request_id: format!("analytics-start-{}", uuid::Uuid::new_v4()),
        servicio: config.servicio.clone(),
        ..Default::default()
    };
    let puerto = config.puerto;
    let servicio = config.servicio.clone();

    let db = match Db::conectar(config.database_url.as_deref().unwrap_or_default()).await {
        Ok(db) => Arc::new(db),
        Err(error) => {
            eprintln!("No se pudo conectar a PostgreSQL: {error}");
            std::process::exit(1);
        }
    };

    let ordenes: Arc<dyn Ordenes> = Arc::new(OrdenesHttp::nueva(
        config.orders_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));

    let rabbit = Arc::new(Rabbit::nueva(
        config.rabbitmq_url.as_deref().unwrap_or_default(),
    ));
    if let Err(error) = rabbit.conectar().await {
        eprintln!("No se pudo conectar a RabbitMQ: {error}");
        std::process::exit(1);
    }
    if rabbit
        .declarar_colas(vec![
            ColaConfig {
                nombre: COLA_ORDERS.to_string(),
                routing_keys: vec!["order.completado".to_string()],
            },
            ColaConfig {
                nombre: COLA_COMISIONES.to_string(),
                routing_keys: vec!["comision.acreditada".to_string()],
            },
        ])
        .await
        .is_err()
    {
        eprintln!("No se pudieron declarar las colas de analytics");
        std::process::exit(1);
    }

    let consumidor = Arc::new(AnalyticsConsumer::nuevo(Arc::clone(&db), ordenes));
    rabbit
        .consumir(COLA_ORDERS, handler_consumidor(Arc::clone(&consumidor)))
        .await;
    rabbit
        .consumir(COLA_COMISIONES, handler_consumidor(consumidor))
        .await;
    rabbit.iniciar_supervisor();
    rabbit.activar_reintento(Duration::from_secs(10));

    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let capacidades: Arc<dyn Capacidades> = Arc::new(CapacidadesHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let tarea_reportes = iniciar_reportes(Arc::clone(&db));

    let estado = AppState::con_dependencias(config, metrics, capacidades, Arc::clone(&db));
    let app = build_router(estado);

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        analytics_service::logs::info(
            "analytics",
            analytics_service::campos! { "msg": "analytics-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            analytics_service::logs::error(
                "analytics",
                analytics_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        tarea_reportes.abort();
        rabbit.cerrar().await;
    })
    .await;
}

/// Ejecuta cada 5 minutos los reportes programados vencidos.
fn iniciar_reportes(db: Arc<Db>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(300));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match analytics_service::reportes::ejecutar_vencidos(&db, 20).await {
                Ok(ejecutados) if ejecutados > 0 => analytics_service::logs::info(
                    "analytics.reportes",
                    analytics_service::campos! {
                        "msg": "Reportes programados ejecutados",
                        "ejecutados": ejecutados,
                    },
                ),
                Ok(_) => {}
                Err(error) => analytics_service::logs::error(
                    "analytics.reportes",
                    analytics_service::campos! {
                        "msg": "Fallo al ejecutar reportes programados",
                        "err": error.mensaje,
                    },
                ),
            }
        }
    })
}

async fn apagar() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminar = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut senal) => {
                senal.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminar = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminar => {},
    }
    analytics_service::logs::info(
        "analytics",
        analytics_service::campos! { "msg": "apagando servicio" },
    );
}
