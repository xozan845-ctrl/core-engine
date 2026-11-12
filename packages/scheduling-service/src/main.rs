//! Arranque del servicio: configuración, PostgreSQL, RabbitMQ, router, tareas
//! de fondo y apagado ordenado.

use scheduling_service::auth::CapacidadesHttp;
use scheduling_service::clients::PartesHttp;
use scheduling_service::config::Config;
use scheduling_service::db::Db;
use scheduling_service::http::build_router;
use scheduling_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use scheduling_service::metrics::Metrics;
use scheduling_service::outbox::Outbox;
use scheduling_service::rabbit::{ColaConfig, Rabbit};
use scheduling_service::scheduling::{Agenda, AgendaPort};
use scheduling_service::state::AppState;
use scheduling_service::tareas::{
    handler_consumidor, iniciar_outbox, iniciar_worker, SchedulingConsumer,
};
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
        request_id: format!("scheduling-start-{}", uuid::Uuid::new_v4()),
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
            nombre: "scheduling.orders".to_string(),
            routing_keys: vec!["order.created".to_string()],
        }])
        .await
        .is_err()
    {
        eprintln!("No se pudo declarar la cola scheduling.orders");
        std::process::exit(1);
    }

    let partes = Arc::new(PartesHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let agenda_concreta = Arc::new(Agenda::nueva(Arc::clone(&db), partes, &outbox_tabla));
    let agenda: Arc<dyn AgendaPort> = agenda_concreta;

    let consumidor = Arc::new(SchedulingConsumer::nuevo(
        Arc::clone(&db),
        Arc::clone(&agenda),
    ));
    rabbit
        .consumir("scheduling.orders", handler_consumidor(consumidor))
        .await;
    rabbit.iniciar_supervisor();
    rabbit.activar_reintento(Duration::from_secs(10));

    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let tarea_outbox = iniciar_outbox(
        Outbox::nueva(outbox_tabla),
        Arc::clone(&db),
        Arc::clone(&rabbit),
        Arc::clone(&metrics),
    );
    let tarea_worker = iniciar_worker(Arc::clone(&agenda));

    let capacidades = Arc::new(CapacidadesHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let estado = AppState::con_dependencias(config, metrics, agenda, capacidades);
    let app = build_router(estado);

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        scheduling_service::logs::info(
            "scheduling",
            scheduling_service::campos! { "msg": "scheduling-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            scheduling_service::logs::error(
                "scheduling",
                scheduling_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        tarea_outbox.abort();
        tarea_worker.abort();
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
    scheduling_service::logs::info(
        "scheduling",
        scheduling_service::campos! { "msg": "apagando servicio" },
    );
}
