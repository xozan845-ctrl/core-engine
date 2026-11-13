//! Arranque del servicio: configuración, PostgreSQL, RabbitMQ, router, tareas
//! de fondo y apagado ordenado.

use market_intelligence_service::agregador::AgregadorPort;
use market_intelligence_service::auth::CapacidadesHttp;
use market_intelligence_service::calidad::CalidadDatos;
use market_intelligence_service::config::Config;
use market_intelligence_service::consumidor::{
    handler_comision, handler_stock, handler_venta, iniciar_outbox, InteligenciaConsumer, COLA_GEO,
    COLA_ORDERS, COLA_STOCK,
};
use market_intelligence_service::db::Db;
use market_intelligence_service::http::build_router;
use market_intelligence_service::inteligencia::{Inteligencia, InteligenciaPort};
use market_intelligence_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use market_intelligence_service::metrics::Metrics;
use market_intelligence_service::outbox::Outbox;
use market_intelligence_service::rabbit::{ColaConfig, Rabbit};
use market_intelligence_service::state::AppState;
use market_intelligence_service::tareas::{iniciar_features, iniciar_vistas};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{}", error);
            std::process::exit(1);
        }
    };

    if let Err(error) = config.validar_arranque() {
        eprintln!("{}", error);
        std::process::exit(1);
    }

    let contexto = ContextoCorrelacion {
        request_id: format!("intelligence-start-{}", uuid::Uuid::new_v4()),
        servicio: config.servicio.clone(),
        ..Default::default()
    };
    let puerto = config.puerto;
    let servicio = config.servicio.clone();
    let outbox_tabla = config.outbox_tabla.clone();

    let db = match Db::conectar(config.database_url.as_deref().unwrap_or_default()).await {
        Ok(db) => Arc::new(db),
        Err(error) => {
            eprintln!("No se pudo conectar a PostgreSQL: {error}");
            std::process::exit(1);
        }
    };

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
                nombre: COLA_GEO.to_string(),
                routing_keys: vec!["venta.geolocalizada".to_string()],
            },
            ColaConfig {
                nombre: COLA_ORDERS.to_string(),
                routing_keys: vec!["comision.acreditada".to_string()],
            },
            ColaConfig {
                nombre: COLA_STOCK.to_string(),
                routing_keys: vec!["stock.updated".to_string()],
            },
        ])
        .await
        .is_err()
    {
        eprintln!("No se pudieron declarar las colas de inteligencia");
        std::process::exit(1);
    }

    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let inteligencia = Arc::new(Inteligencia::nueva(Arc::clone(&db)));
    let inteligencia_port: Arc<dyn InteligenciaPort> = inteligencia.clone();
    let agregador: Arc<dyn AgregadorPort> = inteligencia.clone();
    let calidad = Arc::new(CalidadDatos::nueva(Arc::clone(&metrics), Arc::clone(&db)));

    let consumidor = Arc::new(InteligenciaConsumer::nuevo(
        Arc::clone(&db),
        agregador,
        Arc::clone(&calidad),
    ));
    rabbit
        .consumir(COLA_GEO, handler_venta(Arc::clone(&consumidor)))
        .await;
    rabbit
        .consumir(COLA_ORDERS, handler_comision(Arc::clone(&consumidor)))
        .await;
    rabbit.consumir(COLA_STOCK, handler_stock(consumidor)).await;
    rabbit.iniciar_supervisor();
    rabbit.activar_reintento(Duration::from_secs(10));

    let tarea_outbox = iniciar_outbox(
        Outbox::nueva(&outbox_tabla),
        Arc::clone(&db),
        Arc::clone(&rabbit),
        Arc::clone(&metrics),
    );
    let tarea_features = iniciar_features(Arc::clone(&db));
    let tarea_vistas = iniciar_vistas(Arc::clone(&db), Arc::clone(&metrics));

    let capacidades: Arc<dyn market_intelligence_service::auth::Capacidades> =
        Arc::new(CapacidadesHttp::nueva(
            config.identity_service_url.clone(),
            config.internal_api_key.clone(),
            config.internal_request_timeout_ms,
        ));
    let estado = AppState::con_dependencias(config, metrics, inteligencia_port, capacidades);
    let app = build_router(estado);

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        market_intelligence_service::logs::info(
            "intelligence",
            market_intelligence_service::campos! {
                "msg": "market-intelligence-service listo",
                "puerto": puerto,
            },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            market_intelligence_service::logs::error(
                "intelligence",
                market_intelligence_service::campos! {
                    "msg": "Fallo el servidor HTTP",
                    "err": error.to_string(),
                },
            );
        }
        tarea_outbox.abort();
        tarea_features.abort();
        tarea_vistas.abort();
        rabbit.cerrar().await;
    })
    .await;
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
    market_intelligence_service::logs::info(
        "intelligence",
        market_intelligence_service::campos! { "msg": "apagando servicio" },
    );
}
