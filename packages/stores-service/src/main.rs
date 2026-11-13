//! Arranque del servicio: configuración, PostgreSQL, router y apagado ordenado.

use std::sync::Arc;
use stores_service::auth::CapacidadesHttp;
use stores_service::clients::{CatalogoHttp, ObjetosStorageHttp, TemasIdentityHttp};
use stores_service::config::Config;
use stores_service::db::Db;
use stores_service::http::build_router;
use stores_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use stores_service::metrics::Metrics;
use stores_service::ofertas::Ofertas;
use stores_service::state::AppState;
use stores_service::tiendas::Tiendas;

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
        request_id: format!("stores-start-{}", uuid::Uuid::new_v4()),
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

    let capacidades = Arc::new(CapacidadesHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let temas = Arc::new(TemasIdentityHttp::nueva(
        config.identity_service_url.clone(),
        config.internal_api_key.clone(),
        config.tema_herencia_ttl_ms,
    ));
    let storage = Arc::new(ObjetosStorageHttp::nueva(
        config.storage_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));
    let catalogo = Arc::new(CatalogoHttp::nueva(
        config.catalog_service_url.clone(),
        config.internal_api_key.clone(),
        config.internal_request_timeout_ms,
    ));

    let tiendas = Arc::new(Tiendas::nueva(
        Arc::clone(&db),
        Arc::clone(&temas) as Arc<dyn stores_service::clients::TemasIdentity>,
        Arc::clone(&storage) as Arc<dyn stores_service::clients::ObjetosStorage>,
    ));
    let ofertas = Arc::new(Ofertas::nueva(
        Arc::clone(&db),
        Arc::clone(&catalogo) as Arc<dyn stores_service::clients::Catalogo>,
    ));

    let metrics = Arc::new(Metrics::nueva(servicio.clone()));
    let estado = AppState::con_dependencias(
        config,
        metrics,
        capacidades,
        temas,
        storage,
        catalogo,
        tiendas,
        ofertas,
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
        stores_service::logs::info(
            "stores",
            stores_service::campos! { "msg": "stores-service listo", "puerto": puerto },
        );
        if let Err(error) = axum::serve(listener, app)
            .with_graceful_shutdown(apagar())
            .await
        {
            stores_service::logs::error(
                "stores",
                stores_service::campos! { "msg": "Fallo el servidor HTTP", "err": error.to_string() },
            );
        }
        db.cerrar().await;
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
    stores_service::logs::info(
        "stores",
        stores_service::campos! { "msg": "apagando servicio" },
    );
}
