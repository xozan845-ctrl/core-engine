//! Arranque del servicio: configuración, PostgreSQL, router y apagado ordenado.

use identity_service::config::Config;
use identity_service::crypto::CifradoSecreto;
use identity_service::db::Db;
use identity_service::dominio::api_keys::ApiKeys;
use identity_service::dominio::auditoria::Auditoria;
use identity_service::dominio::auth::Auth;
use identity_service::dominio::capacidades::Capacidades;
use identity_service::dominio::configuracion_operativa::ConfiguracionOperativaService;
use identity_service::dominio::configuracion_sitio::ConfiguracionSitio;
use identity_service::dominio::empresas::Empresas;
use identity_service::dominio::extensiones::Extensiones;
use identity_service::dominio::negocios::Negocios;
use identity_service::dominio::onboarding::Onboarding;
use identity_service::dominio::packs::Packs;
use identity_service::dominio::partes::Partes;
use identity_service::dominio::password_reset::PasswordReset;
use identity_service::dominio::personal::Personal;
use identity_service::dominio::registros::Registros;
use identity_service::dominio::sesiones::Sesiones;
use identity_service::dominio::suscripciones_plataforma::SuscripcionesPlataforma;
use identity_service::dominio::usuarios::Usuarios;
use identity_service::dominio::verificacion::VerificacionEmail;
use identity_service::email::{crear_email, Email};
use identity_service::http::build_router;
use identity_service::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use identity_service::metrics::Metrics;
use identity_service::purga::PurgaExterna;
use identity_service::seed;
use identity_service::state::{AppState, Servicios};
use identity_service::storage::Storage;
use identity_service::stores::Stores;
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
    let clave_mfa = match config.clave_mfa() {
        Ok(clave) => clave,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let cifrado = match CifradoSecreto::nueva(clave_mfa) {
        Ok(cifrado) => Arc::new(cifrado),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    };
    let usuarios = Arc::new(Usuarios::nueva(
        Arc::clone(&db),
        Arc::clone(&storage),
        Arc::clone(&cifrado),
    ));
    let purga = Arc::new(PurgaExterna::nueva(
        config.servicios_purga.clone(),
        config.internal_api_key.clone(),
    ));
    let empresas = Arc::new(Empresas::nueva(
        Arc::clone(&db),
        Arc::clone(&usuarios),
        Arc::clone(&storage),
        Arc::clone(&purga),
        config
            .export_signing_key
            .clone()
            .unwrap_or_else(|| config.jwt_secret.clone()),
    ));
    let empresas_bg = Arc::clone(&empresas);
    tokio::spawn(async move {
        if let Ok(jobs) = empresas_bg.reanudar_exportaciones().await {
            for job_id in jobs {
                let _ = empresas_bg.ejecutar_exportacion(&job_id).await;
            }
        }
    });
    let configuracion = Arc::new(ConfiguracionSitio::nueva(
        Arc::clone(&db),
        Arc::clone(&storage),
        config.platform_tenant_id.clone(),
    ));
    let sesiones = Arc::new(Sesiones::nueva(Arc::clone(&db)));
    let api_keys = Arc::new(ApiKeys::nueva(Arc::clone(&db), &config));
    let capacidades = Arc::new(Capacidades::nueva(Arc::clone(&db)));
    let configuracion_operativa = Arc::new(ConfiguracionOperativaService::nueva(Arc::clone(&db)));
    let packs = Arc::new(Packs::nueva(Arc::clone(&db)));
    let auth = Arc::new(Auth::nueva(
        Arc::clone(&usuarios),
        Arc::clone(&sesiones),
        &config,
    ));
    let auditoria = Arc::new(Auditoria::nueva(Arc::clone(&db)));
    let password_reset = Arc::new(PasswordReset::nueva(
        Arc::clone(&db),
        Arc::clone(&usuarios),
        Arc::clone(&sesiones),
        Arc::clone(&email),
        config.frontend_url.clone(),
    ));
    let verificacion = Arc::new(VerificacionEmail::nueva(
        Arc::clone(&db),
        Arc::clone(&usuarios),
        Arc::clone(&email),
        config.frontend_url.clone(),
    ));
    let stores = Arc::new(Stores::nueva(
        config.stores_service_url.clone(),
        config.internal_api_key.clone(),
        3000,
    ));
    let extensiones = Arc::new(Extensiones::nueva(Arc::clone(&db)));
    let suscripciones = Arc::new(SuscripcionesPlataforma::nueva(
        Arc::clone(&db),
        config.plataforma_plan_codigo.clone(),
        config.plataforma_empresas_gratis,
        config.plataforma_negocios_gratis,
    ));
    let negocios = Arc::new(Negocios::nuevo(Arc::clone(&db), Arc::clone(&suscripciones)));
    let onboarding = Arc::new(Onboarding::nuevo(Arc::clone(&db)));
    let partes = Arc::new(Partes::nueva(Arc::clone(&db)));
    let registros = Arc::new(Registros::nueva(Arc::clone(&db), Arc::clone(&capacidades)));
    let personal = Arc::new(Personal::nueva(
        Arc::clone(&db),
        Arc::clone(&negocios),
        Arc::clone(&auth),
        Arc::clone(&email),
        stores,
        config.frontend_url.clone(),
    ));
    seed::sembrar(&usuarios, &db, &config).await;
    let estado = AppState::con_servicios(
        config,
        metrics,
        Servicios {
            auth,
            auditoria,
            usuarios,
            api_keys,
            capacidades,
            configuracion_operativa,
            configuracion,
            empresas,
            extensiones,
            negocios,
            onboarding,
            personal,
            partes,
            registros,
            suscripciones,
            packs,
            password_reset,
            verificacion,
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
