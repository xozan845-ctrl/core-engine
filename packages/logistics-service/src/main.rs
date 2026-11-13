//! Arranque del servicio: configuración, PostgreSQL, RabbitMQ, router,
//! publicador del Outbox y apagado ordenado.

use logistics_service::auth::CapacidadesHttp;
use logistics_service::clients::OrdersHttp;
use logistics_service::config::Config;
use logistics_service::consumidor::{handler_consumidor, iniciar_outbox, LogisticsConsumer};
use logistics_service::db::Db;
use logistics_service::envios::Envios;
use logistics_service::http::build_router;
use logistics_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use logistics_service::metrics::Metrics;
use logistics_service::outbox::Outbox;
use logistics_service::rabbit::{ColaConfig, Rabbit};
use logistics_service::state::AppState;
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
        request_id: format!("logistics-start-{}", uuid::Uuid::new_v4()),
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
        .declarar_colas(vec![ColaConfig {
            nombre: "logistics.pagos".to_string(),
            routing_keys: vec![
                "payment.procesado".to_string(),
                "order.status.updated".to_string(),
            ],
        }])
        .await
        .is_err()
    {
        eprintln!("No se pudo declarar la cola logistics.pagos");
        std::process::exit(1);
    }

    let orders: Arc<dyn logistics_service::clients::Orders> = Arc::new(OrdersHttp::nueva(
        config.orders_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let capacidades: Arc<dyn logistics_service::auth::Capacidades> =
        Arc::new(CapacidadesHttp::nueva(
            config.identity_service_url.clone(),
            config.internal_api_key.clone(),
            config.internal_request_timeout_ms,
        ));
    let envios = Arc::new(Envios::nueva(
        Arc::clone(&db),
        orders,
        Arc::clone(&capacidades),
        config.platform_logistics_tenant_id.clone(),
        &outbox_tabla,
    ));
    let envios_port: Arc<dyn logistics_service::envios::EnviosPort> = envios.clone();

    let consumidor = Arc::new(LogisticsConsumer::nuevo(
        Arc::clone(&db),
        Arc::clone(&envios_port),
        Outbox::nueva(&outbox_tabla),
    ));
    rabbit
        .consumir("logistics.pagos", handler_consumidor(consumidor))
        .await;
    rabbit.iniciar_supervisor();
    rabbit.activar_reintento(Duration::from_secs(10));

    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let tarea_outbox = iniciar_outbox(
        Outbox::nueva(&outbox_tabla),
        Arc::clone(&db),
        Arc::clone(&rabbit),
        Arc::clone(&metrics),
    );

    let estado = AppState::con_dependencias(config, metrics, envios_port, capacidades);
    let app = build_router(estado);

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        logistics_service::logs::info(
            "logistics",
            logistics_service::campos! { "msg": "logistics-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            logistics_service::logs::error(
                "logistics",
                logistics_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        tarea_outbox.abort();
        rabbit.cerrar().await;
    })
    .await;
}

/// SIGINT/SIGTERM: deja de aceptar peticiones y cierra ordenadamente (R-MS-6).
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
    logistics_service::logs::info(
        "logistics",
        logistics_service::campos! { "msg": "apagando servicio" },
    );
}
