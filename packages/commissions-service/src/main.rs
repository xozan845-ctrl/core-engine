//! Arranque del servicio: configuración, PostgreSQL, RabbitMQ, router, tareas
//! de fondo y apagado ordenado.

use commissions_service::auth::{Capacidades, CapacidadesHttp};
use commissions_service::clients::{Kpis, KpisHttp, Ordenes, OrdenesHttp};
use commissions_service::comisiones::{Comisiones, ComisionesPort};
use commissions_service::config::Config;
use commissions_service::db::Db;
use commissions_service::http::build_router;
use commissions_service::liquidaciones::{Liquidaciones, LiquidacionesPort};
use commissions_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use commissions_service::metrics::Metrics;
use commissions_service::outbox::{iniciar_outbox, Outbox};
use commissions_service::pedidos::{handler_consumidor, handler_liquidacion, PedidosConsumer};
use commissions_service::rabbit::{ColaConfig, Rabbit};
use commissions_service::reportes::{Reportes, ReportesPort};
use commissions_service::state::AppState;
use commissions_service::ventas::{Ventas, VentasPort};
use commissions_service::worker::iniciar_worker;
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
        request_id: format!("commissions-start-{}", uuid::Uuid::new_v4()),
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
    if let Err(error) = rabbit
        .declarar_colas(vec![
            ColaConfig {
                nombre: "commissions.pedidos".to_string(),
                routing_keys: vec![
                    "order.completado".to_string(),
                    "payment.procesado".to_string(),
                    "devolucion.solicitada".to_string(),
                ],
            },
            ColaConfig {
                nombre: "commissions.liquidacion".to_string(),
                routing_keys: vec!["comision.acreditada".to_string()],
            },
        ])
        .await
    {
        eprintln!("No se pudieron declarar las colas de commissions: {error}");
        std::process::exit(1);
    }

    let ordenes: Arc<dyn Ordenes> = Arc::new(OrdenesHttp::nueva(
        config.orders_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let kpis: Arc<dyn Kpis> = Arc::new(KpisHttp::nueva(
        config.catalog_service_url.clone(),
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));

    let comisiones_concretas = Arc::new(Comisiones::nueva(Arc::clone(&db), config.tasa_comision()));
    let comisiones: Arc<dyn ComisionesPort> = comisiones_concretas.clone();
    let liquidaciones_concretas = Arc::new(Liquidaciones::nueva(Arc::clone(&db)));
    let liquidaciones: Arc<dyn LiquidacionesPort> = liquidaciones_concretas.clone();
    let ventas: Arc<dyn VentasPort> =
        Arc::new(Ventas::nueva(Arc::clone(&db), Arc::clone(&comisiones)));
    let reportes: Arc<dyn ReportesPort> =
        Arc::new(Reportes::nueva(Arc::clone(&db), Arc::clone(&kpis)));
    let capacidades: Arc<dyn Capacidades> = Arc::new(CapacidadesHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));

    let outbox = Outbox::nueva(outbox_tabla.clone());
    let consumidor = Arc::new(PedidosConsumer::nuevo(
        Arc::clone(&db),
        Arc::clone(&comisiones_concretas),
        outbox.clone(),
        Arc::clone(&ordenes),
    ));
    rabbit
        .consumir(
            "commissions.pedidos",
            handler_consumidor(Arc::clone(&consumidor)),
        )
        .await;
    rabbit
        .consumir(
            "commissions.liquidacion",
            handler_liquidacion(Arc::clone(&consumidor)),
        )
        .await;
    rabbit.iniciar_supervisor();
    rabbit.activar_reintento(Duration::from_secs(10));

    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let tarea_outbox = iniciar_outbox(
        outbox,
        Arc::clone(&db),
        Arc::clone(&rabbit),
        Arc::clone(&metrics),
    );
    let tarea_worker = iniciar_worker(Arc::clone(&liquidaciones));

    let estado = AppState::con_dependencias(
        config,
        Arc::clone(&metrics),
        comisiones,
        liquidaciones,
        ventas,
        reportes,
        capacidades,
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
        commissions_service::logs::info(
            "commissions",
            commissions_service::campos! { "msg": "commissions-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            commissions_service::logs::error(
                "commissions",
                commissions_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
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
    commissions_service::logs::info(
        "commissions",
        commissions_service::campos! { "msg": "apagando servicio" },
    );
}
