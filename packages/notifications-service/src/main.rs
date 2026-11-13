//! Arranque del servicio: configuración, PostgreSQL, RabbitMQ, correo, router y
//! apagado ordenado.

use notifications_service::clientes::ClientesHttp;
use notifications_service::config::Config;
use notifications_service::consumidor::{handler_consumidor, ConsumidorNotificaciones};
use notifications_service::db::Db;
use notifications_service::email::crear_proveedor;
use notifications_service::http::build_router;
use notifications_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use notifications_service::metrics::Metrics;
use notifications_service::notificaciones::{Notificaciones, NotificacionesPort};
use notifications_service::rabbit::{ColaConfig, Rabbit};
use notifications_service::state::AppState;
use std::sync::Arc;
use std::time::Duration;

const COLA_NOTIFICACIONES: &str = "notifications.todos";
const ROUTING_KEYS: [&str; 7] = [
    "order.created",
    "order.status.updated",
    "payment.procesado",
    "order.completado",
    "shipment.started",
    "devolucion.solicitada",
    "stock.fallido",
];

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
        request_id: format!("notifications-start-{}", uuid::Uuid::new_v4()),
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
        &config.servicio,
    ));
    if let Err(error) = rabbit.conectar().await {
        eprintln!("No se pudo conectar a RabbitMQ: {error}");
        std::process::exit(1);
    }
    if rabbit
        .declarar_colas(vec![ColaConfig {
            nombre: COLA_NOTIFICACIONES.to_string(),
            routing_keys: ROUTING_KEYS.iter().map(|clave| clave.to_string()).collect(),
        }])
        .await
        .is_err()
    {
        eprintln!("No se pudo declarar la cola {COLA_NOTIFICACIONES}");
        std::process::exit(1);
    }

    let email = match crear_proveedor(&config.email) {
        Ok(proveedor) => proveedor,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let clientes = Arc::new(ClientesHttp::nueva(
        config.orders_service_url.clone(),
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let notificaciones: Arc<dyn NotificacionesPort> =
        Arc::new(Notificaciones::nueva(clientes, email));
    let consumidor = Arc::new(ConsumidorNotificaciones::nuevo(
        Arc::clone(&db),
        notificaciones,
    ));
    rabbit
        .consumir(COLA_NOTIFICACIONES, handler_consumidor(consumidor))
        .await;
    notifications_service::consumidor::anunciar_consumidor();
    rabbit.iniciar_supervisor();
    rabbit.activar_reintento(Duration::from_secs(10));

    let metrics = Arc::new(Metrics::nueva(servicio));
    let estado = AppState {
        config: Arc::new(config),
        metrics: Arc::clone(&metrics),
    };
    let app = build_router(estado);

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        notifications_service::logs::info(
            "notifications-service",
            notifications_service::campos! { "msg": "notifications-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            notifications_service::logs::error(
                "notifications-service",
                notifications_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        db.cerrar().await;
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
    notifications_service::logs::info(
        "notifications-service",
        notifications_service::campos! { "msg": "apagando servicio" },
    );
}
