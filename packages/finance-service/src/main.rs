//! Arranque del servicio: configuración, PostgreSQL, RabbitMQ, consumidor,
//! publicador del Outbox, planificador de declaraciones y apagado ordenado.

use chrono::Datelike;
use finance_service::auth::CapacidadesHttp;
use finance_service::comercio::Comercio;
use finance_service::config::Config;
use finance_service::consumidor::{handler_consumidor, Consumidor};
use finance_service::contabilidad::Contabilidad;
use finance_service::db::Db;
use finance_service::facturacion::{Facturacion, OrdenesHttp};
use finance_service::finanzas::alertas::Alertas;
use finance_service::finanzas::kpis::{Kpis, VendedoresHttp};
use finance_service::finanzas::proyecciones::Proyecciones;
use finance_service::http::build_router;
use finance_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use finance_service::metrics::Metrics;
use finance_service::outbox::Outbox;
use finance_service::rabbit::{ColaConfig, Rabbit};
use finance_service::state::AppState;
use finance_service::tributacion::Tributacion;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

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
        request_id: format!("finance-start-{}", uuid::Uuid::new_v4()),
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
            nombre: "finance.pedidos".to_string(),
            routing_keys: vec![
                "order.created".to_string(),
                "payment.procesado".to_string(),
                "comision.acreditada".to_string(),
                "devolucion.solicitada".to_string(),
            ],
        }])
        .await
        .is_err()
    {
        eprintln!("No se pudo declarar la cola finance.pedidos");
        std::process::exit(1);
    }

    let contabilidad = Arc::new(Contabilidad::nueva(
        Arc::clone(&db),
        Outbox::nueva(outbox_tabla.clone()),
    ));
    let ordenes = Arc::new(OrdenesHttp::nueva(
        config.orders_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let vendedores = Arc::new(VendedoresHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let capacidades: Arc<dyn finance_service::auth::Capacidades> =
        Arc::new(CapacidadesHttp::nueva(
            config.identity_service_url.clone(),
            config.internal_api_key.clone(),
            config.internal_request_timeout_ms,
        ));
    let tributacion = Arc::new(Tributacion::nueva(
        Arc::clone(&db),
        Outbox::nueva(outbox_tabla.clone()),
        Arc::clone(&contabilidad),
    ));
    let facturacion = Arc::new(Facturacion::nueva(
        Arc::clone(&db),
        Outbox::nueva(outbox_tabla.clone()),
        Arc::clone(&contabilidad),
        ordenes.clone(),
        config.emitir_comprobantes_fiscales,
    ));
    let comercio = Arc::new(Comercio::nuevo(Arc::clone(&db)));
    let proyecciones = Arc::new(Proyecciones::nueva(Arc::clone(&db)));
    let kpis = Arc::new(Kpis::nueva(Arc::clone(&db), vendedores));
    let alertas = Arc::new(Alertas::nueva(
        Arc::clone(&kpis),
        Arc::clone(&proyecciones),
        Arc::clone(&db),
    ));

    let consumidor = Arc::new(Consumidor::nuevo(
        Arc::clone(&db),
        Arc::clone(&contabilidad),
        ordenes,
        config.commission_rate,
    ));
    rabbit
        .consumir("finance.pedidos", handler_consumidor(consumidor))
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
    let tarea_declaraciones = iniciar_planificador(Arc::clone(&tributacion));

    let estado = AppState::nueva(config, Metrics::nueva(servicio))
        .con_capacidades(capacidades)
        .con_servicios(
            comercio,
            contabilidad,
            tributacion,
            facturacion,
            proyecciones,
            kpis,
            alertas,
            Arc::clone(&metrics),
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
        finance_service::logs::info(
            "finance",
            finance_service::campos! { "msg": "finance-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            finance_service::logs::error(
                "finance",
                finance_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        tarea_outbox.abort();
        tarea_declaraciones.abort();
        rabbit.cerrar().await;
    })
    .await;
}

fn iniciar_outbox(
    outbox: Outbox,
    db: Arc<Db>,
    rabbit: Arc<Rabbit>,
    metrics: Arc<Metrics>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        loop {
            ticker.tick().await;
            let _ = outbox.publicar_pendientes(&db, &rabbit, &metrics).await;
        }
    })
}

/// Genera las declaraciones del mes anterior el día 1 tras las 00:10 (Ley 822),
/// con una sola ejecución por periodo.
fn iniciar_planificador(tributacion: Arc<Tributacion>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let ejecutado: Mutex<Option<String>> = Mutex::new(None);
        let mut ticker = tokio::time::interval(Duration::from_secs(60));
        loop {
            ticker.tick().await;
            let ahora = chrono::Local::now();
            if ahora.day() != 1 || ahora.format("%H:%M").to_string().as_str() < "00:10" {
                continue;
            }
            let periodo = ahora.format("%Y-%m").to_string();
            let mut guardado = ejecutado.lock().await;
            if guardado.as_deref() == Some(periodo.as_str()) {
                continue;
            }
            *guardado = Some(periodo);
            drop(guardado);
            if let Err(error) = tributacion
                .generar_declaraciones_del_mes_anterior(ahora.date_naive())
                .await
            {
                finance_service::logs::error(
                    "finance.tributacion",
                    finance_service::campos! {
                        "msg": "Fallo la generacion mensual de declaraciones",
                        "err": error.mensaje,
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
    finance_service::logs::info(
        "finance",
        finance_service::campos! { "msg": "apagando servicio" },
    );
}
