//! Arranque del servicio: configuración, PostgreSQL, router y apagado ordenado.

use field_service::auth::CapacidadesHttp;
use field_service::clients::PartesHttp;
use field_service::config::Config;
use field_service::db::Db;
use field_service::field::FieldService;
use field_service::http::build_router;
use field_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use field_service::metrics::Metrics;
use field_service::state::AppState;
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
        request_id: format!("field-start-{}", uuid::Uuid::new_v4()),
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

    let partes = Arc::new(PartesHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let capacidades = Arc::new(CapacidadesHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let field = Arc::new(FieldService::nuevo(Arc::clone(&db), partes));
    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let estado = AppState::con_dependencias(config, metrics, field, capacidades);
    let app = build_router(estado);

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        field_service::logs::info(
            "field",
            field_service::campos! { "msg": "field-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            field_service::logs::error(
                "field",
                field_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
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
}
