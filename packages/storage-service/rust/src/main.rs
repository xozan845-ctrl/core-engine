//! Arranque del storage-service: configuración, PostgreSQL, RabbitMQ, S3/MinIO,
//! router, poller de outbox y apagado ordenado.

use std::sync::Arc;
use std::time::Duration;
use storage_service::almacen::{AlmacenObjetos, AlmacenS3};
use storage_service::config::Config;
use storage_service::db::Db;
use storage_service::http::build_router;
use storage_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use storage_service::metrics::Metrics;
use storage_service::outbox::Outbox;
use storage_service::rabbit::Rabbit;
use storage_service::state::AppState;
use storage_service::storage::buckets::Buckets;
use storage_service::storage::objetos::Objetos;

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
    let puerto = config.puerto;
    let servicio = config.servicio.clone();
    let outbox_tabla = config.outbox_tabla.clone();
    let contexto = ContextoCorrelacion {
        request_id: format!("storage-start-{}", uuid::Uuid::new_v4()),
        servicio: servicio.clone(),
        ..Default::default()
    };

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
    rabbit.iniciar_supervisor();

    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let almacen: Arc<dyn AlmacenObjetos> = Arc::new(AlmacenS3::nueva(&config));
    let config = Arc::new(config);
    let buckets = Arc::new(Buckets::nueva(
        Arc::clone(&db),
        Arc::clone(&almacen),
        Arc::clone(&config),
    ));
    let outbox = Arc::new(Outbox::nueva(outbox_tabla));
    let objetos = Arc::new(Objetos::nuevo(
        Arc::clone(&db),
        Arc::clone(&buckets),
        Arc::clone(&outbox),
        Arc::clone(&almacen),
        Arc::clone(&config),
    ));

    let outbox_tarea = {
        let db = Arc::clone(&db);
        let rabbit = Arc::clone(&rabbit);
        let metrics = Arc::clone(&metrics);
        let outbox = Arc::clone(&outbox);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let _ = outbox.publicar_pendientes(&db, &rabbit, &metrics).await;
            }
        })
    };

    let estado =
        AppState::con_dependencias((*config).clone(), Arc::clone(&metrics), buckets, objetos);
    let app = build_router(estado);
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        storage_service::logs::info(
            "storage",
            storage_service::campos! { "msg": "storage-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            storage_service::logs::error(
                "storage",
                storage_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        outbox_tarea.abort();
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
    storage_service::logs::info(
        "storage",
        storage_service::campos! { "msg": "apagando servicio" },
    );
}
