//! Arranque de `inventory-service`: configuración, PostgreSQL, RabbitMQ,
//! consumidor de la saga, Outbox y apagado ordenado.

use inventory_service::auth::CapacidadesHttp;
use inventory_service::config::Config;
use inventory_service::consumidor::{cola, handler_consumidor, PedidosConsumer};
use inventory_service::db::Db;
use inventory_service::http::build_router;
use inventory_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use inventory_service::metrics::Metrics;
use inventory_service::outbox::Outbox;
use inventory_service::rabbit::Rabbit;
use inventory_service::state::AppState;
use std::sync::Arc;

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
        request_id: format!("inventory-start-{}", uuid::Uuid::new_v4()),
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

    let rabbit = Arc::new(Rabbit::nueva(
        config.rabbitmq_url.as_deref().unwrap_or_default(),
    ));
    if let Err(error) = rabbit.conectar().await {
        eprintln!("No se pudo conectar a RabbitMQ: {error}");
        std::process::exit(1);
    }

    let outbox = Arc::new(Outbox::nueva(config.outbox_tabla.clone()));
    if rabbit.declarar_colas(vec![cola()]).await.is_err() {
        eprintln!("No se pudo declarar la cola inventory.pedidos");
        std::process::exit(1);
    }
    let consumidor = Arc::new(PedidosConsumer::nuevo(Arc::clone(&db), Arc::clone(&outbox)));
    rabbit
        .consumir("inventory.pedidos", handler_consumidor(consumidor))
        .await;
    rabbit.iniciar_supervisor();
    rabbit.activar_reintento(std::time::Duration::from_secs(10));

    let capacidades = Arc::new(CapacidadesHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let tarea_outbox = iniciar_outbox(
        Arc::clone(&outbox),
        Arc::clone(&db),
        Arc::clone(&rabbit),
        Arc::clone(&metrics),
    );

    let estado = AppState::con_db(config, Arc::clone(&metrics), Arc::clone(&db), capacidades);
    let app = build_router(estado);

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        inventory_service::logs::info(
            "inventory-service",
            inventory_service::campos! { "msg": "inventory-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            inventory_service::logs::error(
                "inventory-service",
                inventory_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        tarea_outbox.abort();
        rabbit.cerrar().await;
        db.cerrar().await;
    })
    .await;
}

fn iniciar_outbox(
    outbox: Arc<Outbox>,
    db: Arc<Db>,
    rabbit: Arc<Rabbit>,
    metrics: Arc<Metrics>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            ticker.tick().await;
            if let Err(error) = outbox.publicar_pendientes(&db, &rabbit, &metrics).await {
                inventory_service::logs::error(
                    "outbox",
                    inventory_service::campos! {
                        "msg": "Fallo al publicar pendientes",
                        "err": format!("{error:?}"),
                    },
                );
            }
        }
    })
}

async fn apagar() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            eprintln!("No se pudo escuchar SIGINT: {error}");
        }
    };

    #[cfg(unix)]
    let terminar = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut senal) => {
                senal.recv().await;
            }
            Err(error) => eprintln!("No se pudo escuchar SIGTERM: {error}"),
        }
    };

    #[cfg(not(unix))]
    let terminar = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminar => {},
    }
}
