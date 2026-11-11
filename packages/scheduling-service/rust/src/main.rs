//! `scheduling-service` en Rust — reescritura con paridad de contrato.
//!
//! Ver `docs/adr/0004-scheduling-service-en-rust.md`. Este andamiaje conserva
//! configuración, envelope, errores, correlación, métricas y apagado ordenado;
//! las rutas de agenda y la infraestructura de datos llegan en los PR 3-5.

// El andamiaje expone piezas (auth, errores, métricas) que aún no tienen rutas
// que las consuman; el `allow` desaparece al completar los PR 3-6.
#![allow(dead_code)]

mod auth;
mod config;
mod errors;
mod http;
mod logs;
mod metrics;
mod state;

use crate::config::Config;
use crate::http::build_router;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use crate::metrics::Metrics;
use crate::state::AppState;

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
    let estado = AppState::nueva(config, Metrics::nueva(servicio.clone()));
    let app = build_router(estado);

    let listener = match tokio::net::TcpListener::bind(("0.0.0.0", puerto)).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("No se pudo abrir el puerto {puerto}: {error}");
            std::process::exit(1);
        }
    };

    ejecutar_con_contexto(contexto, async {
        crate::logs::info(
            "scheduling",
            crate::campos! { "msg": "scheduling-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            crate::logs::error(
                "scheduling",
                crate::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
            std::process::exit(1);
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
    crate::logs::info("scheduling", crate::campos! { "msg": "apagando servicio" });
}
