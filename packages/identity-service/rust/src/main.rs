//! Arranque del servicio: configuración, PostgreSQL, router y apagado ordenado.

use identity_service::config::Config;
use identity_service::db::Db;
use identity_service::dominio::api_keys::ApiKeys;
use identity_service::dominio::auth::Auth;
use identity_service::dominio::capacidades::Capacidades;
use identity_service::dominio::configuracion_sitio::ConfiguracionSitio;
use identity_service::dominio::empresas::Empresas;
use identity_service::dominio::packs::Packs;
use identity_service::dominio::password_reset::PasswordReset;
use identity_service::dominio::sesiones::Sesiones;
use identity_service::dominio::usuarios::Usuarios;
use identity_service::email::{crear_email, Email};
use identity_service::http::build_router;
use identity_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use identity_service::metrics::Metrics;
use identity_service::state::{AppState, Servicios};
use identity_service::storage::Storage;
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
        request_id: format!("identity-start-{}", uuid::Uuid::new_v4()),
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

    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let email: Arc<dyn Email> = match crear_email(&config) {
        Ok(email) => Arc::from(email),
        Err(error) => {
            eprintln!("No se pudo configurar el correo: {error}");
            std::process::exit(1);
        }
    };
    let storage = Arc::new(Storage::nueva(
        config.storage_service_url.clone(),
        config.internal_api_key.clone(),
    ));
    let usuarios = Arc::new(Usuarios::nueva(Arc::clone(&db), Arc::clone(&storage)));
    let empresas = Arc::new(Empresas::nueva(
        Arc::clone(&db),
        Arc::clone(&usuarios),
        Arc::clone(&storage),
    ));
    let configuracion = Arc::new(ConfiguracionSitio::nueva(
        Arc::clone(&db),
        Arc::clone(&storage),
        config.platform_tenant_id.clone(),
    ));
    let sesiones = Arc::new(Sesiones::nueva(Arc::clone(&db)));
    let api_keys = Arc::new(ApiKeys::nueva(Arc::clone(&db), &config));
    let capacidades = Arc::new(Capacidades::nueva(Arc::clone(&db)));
    let packs = Arc::new(Packs::nueva(Arc::clone(&db)));
    let auth = Arc::new(Auth::nueva(
        Arc::clone(&usuarios),
        Arc::clone(&sesiones),
        &config,
    ));
    let password_reset = Arc::new(PasswordReset::nueva(
        Arc::clone(&db),
        Arc::clone(&usuarios),
        Arc::clone(&sesiones),
        email,
        config.frontend_url.clone(),
    ));
    let estado = AppState::con_servicios(
        config,
        metrics,
        Servicios {
            auth,
            usuarios,
            api_keys,
            capacidades,
            configuracion,
            empresas,
            packs,
            password_reset,
        },
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
        identity_service::logs::info(
            "identity",
            identity_service::campos! { "msg": "identity-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            identity_service::logs::error(
                "identity",
                identity_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        drop(db);
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
    identity_service::logs::info(
        "identity",
        identity_service::campos! { "msg": "apagando servicio" },
    );
}
