//! Arranque del servicio: configuración, PostgreSQL, RabbitMQ, router y
//! apagado ordenado.

use catalog_service::auth::CapacidadesHttp;
use catalog_service::componentes::Componentes;
use catalog_service::config::Config;
use catalog_service::consumidor::{cola, handler_consumidor, PedidosConsumer};
use catalog_service::db::Db;
use catalog_service::http::build_router;
use catalog_service::inventario::Inventario;
use catalog_service::items::Items;
use catalog_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use catalog_service::metrics::Metrics;
use catalog_service::operaciones::Operaciones;
use catalog_service::outbox::Outbox;
use catalog_service::precios::Precios;
use catalog_service::productos::Productos;
use catalog_service::rabbit::Rabbit;
use catalog_service::series::Series;
use catalog_service::servicios::Servicios;
use catalog_service::state::AppState;
use catalog_service::tarifas::Tarifas;
use catalog_service::ubicaciones::Ubicaciones;
use std::sync::Arc;

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
        request_id: format!("catalog-start-{}", uuid::Uuid::new_v4()),
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

    let capacidades = Arc::new(CapacidadesHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));

    let outbox = Arc::new(Outbox::nueva(config.outbox_tabla.clone()));
    let items = Arc::new(Items::nueva(Arc::clone(&db)));
    let inventario = Arc::new(Inventario::nueva(Arc::clone(&db)));
    let ubicaciones = Arc::new(Ubicaciones::nueva(Arc::clone(&db), Arc::clone(&items)));
    let productos = Arc::new(Productos::nueva(
        Arc::clone(&db),
        Some(Arc::clone(&outbox)),
        Some(Arc::clone(&items)),
    ));
    let servicios = Arc::new(Servicios::con_productos(
        Arc::clone(&db),
        Arc::clone(&productos),
        Some(Arc::clone(&items)),
    ));
    let precios = Arc::new(Precios::nueva(Arc::clone(&db)));
    let tarifas = Arc::new(Tarifas::nueva(Arc::clone(&db)));
    let series = Arc::new(Series::nueva(Arc::clone(&db)));
    let componentes = Arc::new(Componentes::nueva(Arc::clone(&db), Arc::clone(&items)));
    let operaciones = Arc::new(Operaciones::nueva(
        Arc::clone(&db),
        Some(Arc::clone(&outbox)),
    ));

    if rabbit.declarar_colas(vec![cola()]).await.is_err() {
        eprintln!("No se pudo declarar la cola catalog.pedidos");
        std::process::exit(1);
    }
    let consumidor = Arc::new(PedidosConsumer::nuevo(
        Arc::clone(&db),
        Arc::clone(&outbox),
        Arc::clone(&productos),
    ));
    rabbit
        .consumir("catalog.pedidos", handler_consumidor(consumidor))
        .await;
    rabbit.iniciar_supervisor();
    rabbit.activar_reintento(std::time::Duration::from_secs(10));

    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let tarea_outbox = iniciar_outbox(
        Arc::clone(&outbox),
        Arc::clone(&db),
        Arc::clone(&rabbit),
        Arc::clone(&metrics),
    );

    let estado = AppState::con_dependencias(
        config,
        metrics,
        Arc::clone(&db),
        capacidades,
        items,
        inventario,
        ubicaciones,
        productos,
        servicios,
        precios,
        tarifas,
        series,
        componentes,
        operaciones,
    );
    let app = build_router(estado);

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        catalog_service::logs::info(
            "catalog",
            catalog_service::campos! { "msg": "catalog-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            catalog_service::logs::error(
                "catalog",
                catalog_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        tarea_outbox.abort();
        rabbit.cerrar().await;
        db.cerrar().await;
    })
    .await;
}

/// Poller del Outbox: publica pendientes cada segundo (ADR-03).
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
                catalog_service::logs::error(
                    "outbox",
                    catalog_service::campos! {
                        "msg": "Fallo al publicar pendientes",
                        "err": format!("{error:?}"),
                    },
                );
            }
        }
    })
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
    catalog_service::logs::info(
        "catalog",
        catalog_service::campos! { "msg": "apagando servicio" },
    );
}
