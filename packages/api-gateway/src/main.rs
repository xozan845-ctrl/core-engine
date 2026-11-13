//! Arranque del api-gateway en Rust.

use api_gateway::config::Config;
use api_gateway::http::build_router;
use api_gateway::logs;
use api_gateway::metrics::Metrics;
use api_gateway::state::AppState;

#[tokio::main]
async fn main() {
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let puerto = config.puerto;
    let servicio = config.servicio.clone();
    let estado = AppState::nueva(config, Metrics::nueva(servicio.clone()));
    let app = build_router(estado);
    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };
    logs::info(
        "gateway",
        api_gateway::campos! { "msg": "api-gateway listo", "puerto": puerto, "docs": "/docs" },
    );
    if let Err(error) = axum::serve(listener, app)
        .with_graceful_shutdown(apagar())
        .await
    {
        logs::error(
            "gateway",
            api_gateway::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
        );
    }
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
}
