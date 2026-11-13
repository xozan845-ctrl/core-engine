//! Arranque del servicio: configuración, PostgreSQL, RabbitMQ, router,
//! scheduler de suscripciones, publicador del Outbox y apagado ordenado.

use orders_service::carrito::Carrito;
use orders_service::checkout::Checkout;
use orders_service::clients::Clientes;
use orders_service::config::Config;
use orders_service::consumer::PedidosConsumer;
use orders_service::contratos::Contratos;
use orders_service::db::Db;
use orders_service::entregas::EntregasDigitales;
use orders_service::http::build_router;
use orders_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use orders_service::metrics::Metrics;
use orders_service::outbox::Outbox;
use orders_service::pedidos::Pedidos;
use orders_service::rabbit::{ColaConfig, Handler, Rabbit};
use orders_service::salon::Salon;
use orders_service::state::AppState;
use orders_service::suscripciones::Suscripciones;
use orders_service::views::OrderViewRepository;
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
        request_id: format!("orders-start-{}", uuid::Uuid::new_v4()),
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
            nombre: "orders.resultados".to_string(),
            routing_keys: vec![
                "stock.reservado".to_string(),
                "stock.fallido".to_string(),
                "stock.reintegrado".to_string(),
            ],
        }])
        .await
        .is_err()
    {
        eprintln!("No se pudo declarar la cola orders.resultados");
        std::process::exit(1);
    }

    let clientes: Arc<dyn orders_service::clients::ClientesPort> = Clientes::nueva(&config);
    let carrito = Arc::new(Carrito::nueva(Arc::clone(&db), Arc::clone(&clientes)));
    let vistas = Arc::new(OrderViewRepository);
    let pedidos = Arc::new(Pedidos {
        tasa_comision: config.tasa_comision,
    });
    let entregas = Arc::new(EntregasDigitales::nueva(Arc::clone(&db)));
    let checkout = Arc::new(Checkout {
        db: Arc::clone(&db),
        clientes: Arc::clone(&clientes),
        carrito: Arc::clone(&carrito),
        vistas: Arc::clone(&vistas),
        pedidos: Arc::clone(&pedidos),
        outbox: Outbox::nueva(&outbox_tabla),
        tasa_comision: config.tasa_comision,
    });
    let contratos = Arc::new(Contratos::nueva(Arc::clone(&db)));
    let suscripciones = Arc::new(Suscripciones::nueva(Arc::clone(&db), Arc::clone(&checkout)));
    let salon = Arc::new(Salon::nueva(Arc::clone(&db)));

    let consumidor = Arc::new(PedidosConsumer::nuevo(
        Arc::clone(&db),
        Arc::clone(&checkout),
    ));
    rabbit
        .consumir("orders.resultados", handler_consumidor(consumidor))
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
    let tarea_suscripciones = iniciar_suscripciones(Arc::clone(&suscripciones));

    let estado = AppState::con_dependencias(
        config,
        metrics,
        db,
        clientes,
        checkout,
        carrito,
        vistas,
        entregas,
        pedidos,
        contratos,
        suscripciones,
        salon,
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
        orders_service::logs::info(
            "orders",
            orders_service::campos! { "msg": "orders-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            orders_service::logs::error(
                "orders",
                orders_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        tarea_outbox.abort();
        tarea_suscripciones.abort();
        rabbit.cerrar().await;
    })
    .await;
}

fn handler_consumidor(consumidor: Arc<PedidosConsumer>) -> Handler {
    Arc::new(move |payload| {
        let consumidor = Arc::clone(&consumidor);
        Box::pin(async move { consumidor.manejar(payload).await })
    })
}

fn iniciar_outbox(
    outbox: Outbox,
    db: Arc<Db>,
    rabbit: Arc<Rabbit>,
    metrics: Arc<Metrics>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            if let Err(error) = outbox.publicar_pendientes(&db, &rabbit, &metrics).await {
                orders_service::logs::error(
                    "outbox",
                    orders_service::campos! {
                        "msg": "Fallo al publicar eventos outbox",
                        "err": error.mensaje,
                    },
                );
            }
        }
    })
}

/// Equivalente a `@Cron(EVERY_HOUR)`: renueva suscripciones vencidas cada hora.
fn iniciar_suscripciones(suscripciones: Arc<Suscripciones>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(3600));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            match suscripciones.renovar_vencidas().await {
                Ok(renovadas) if renovadas > 0 => orders_service::logs::info(
                    "orders.suscripciones",
                    orders_service::campos! {
                        "msg": "Suscripciones renovadas",
                        "renovadas": renovadas,
                    },
                ),
                Ok(_) => {}
                Err(error) => orders_service::logs::error(
                    "orders.suscripciones",
                    orders_service::campos! {
                        "msg": "Fallo al renovar suscripciones",
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
    orders_service::logs::info(
        "orders",
        orders_service::campos! { "msg": "apagando servicio" },
    );
}
