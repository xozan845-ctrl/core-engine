//! Pruebas de integración contra PostgreSQL (solo si `TEST_DATABASE_URL` está
//! definida y el esquema `notifications` existe o puede crearse). El runner de
//! CI las ejecuta con la BD efímera.

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use notifications_service::config::Config;
use notifications_service::consumidor::ConsumidorNotificaciones;
use notifications_service::db::Db;
use notifications_service::errors::ErrorNotificacion;
use notifications_service::http::build_router;
use notifications_service::metrics::Metrics;
use notifications_service::notificaciones::NotificacionesPort;
use notifications_service::plantillas_db::Plantillas;
use notifications_service::state::AppState;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

static MIGRACION: OnceLock<Mutex<()>> = OnceLock::new();

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

struct Procesador {
    veces: AtomicUsize,
    falla: bool,
}

#[async_trait]
impl NotificacionesPort for Procesador {
    async fn procesar(&self, _evento: &Value) -> Result<(), ErrorNotificacion> {
        self.veces.fetch_add(1, Ordering::SeqCst);
        if self.falla {
            return Err(ErrorNotificacion::Correo("SMTP caído".to_string()));
        }
        Ok(())
    }
}

fn evento(event_id: Uuid) -> Value {
    json!({
        "event_id": event_id.to_string(),
        "tipo": "payment.procesado",
        "ocurrido_en": "2026-11-11T00:00:00.000Z",
        "data": { "order_id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee" },
    })
}

async fn preparar(url: &str) -> (Arc<Db>, PgPool) {
    let pool = PgPool::connect(url).await.expect("postgres");
    // `CREATE SCHEMA IF NOT EXISTS` no es seguro ante dos tests concurrentes:
    // se serializa solo la migración y cada test usa un `event_id` propio.
    let _guard = MIGRACION.get_or_init(|| Mutex::new(())).lock().await;
    sqlx::raw_sql(include_str!("../../../infra/db/init/56_notifications.sql"))
        .execute(&pool)
        .await
        .expect("migracion notifications");
    sqlx::raw_sql(include_str!(
        "../../../infra/db/init/99_notificaciones_plantillas.sql"
    ))
    .execute(&pool)
    .await
    .expect("migracion plantillas");
    let db = Arc::new(Db::conectar(url).await.expect("pool del servicio"));
    (db, pool)
}

#[tokio::test]
async fn debe_procesar_una_sola_vez_por_event_id() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let event_id = Uuid::new_v4();
    let procesador = Arc::new(Procesador {
        veces: AtomicUsize::new(0),
        falla: false,
    });
    let consumidor = ConsumidorNotificaciones::nuevo(
        Arc::clone(&db),
        Arc::clone(&procesador) as Arc<dyn NotificacionesPort>,
    );

    consumidor.manejar(evento(event_id)).await.expect("primera");
    consumidor.manejar(evento(event_id)).await.expect("segunda");

    assert_eq!(procesador.veces.load(Ordering::SeqCst), 1);
    let fila: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT event_id, tipo FROM notifications.eventos_procesados WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_optional(&pool)
    .await
    .expect("consulta");
    assert_eq!(fila, Some((event_id, "payment.procesado".to_string())));

    db.cerrar().await;
    pool.close().await;
}

#[tokio::test]
async fn no_debe_marcar_el_evento_si_el_envio_falla() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let event_id = Uuid::new_v4();
    let procesador = Arc::new(Procesador {
        veces: AtomicUsize::new(0),
        falla: true,
    });
    let consumidor = ConsumidorNotificaciones::nuevo(
        Arc::clone(&db),
        Arc::clone(&procesador) as Arc<dyn NotificacionesPort>,
    );

    let error = consumidor
        .manejar(evento(event_id))
        .await
        .expect_err("fallo de correo");
    assert_eq!(error.mensaje(), "SMTP caído");

    let fila: Option<(Uuid,)> =
        sqlx::query_as("SELECT event_id FROM notifications.eventos_procesados WHERE event_id = $1")
            .bind(event_id)
            .fetch_optional(&pool)
            .await
            .expect("consulta");
    assert_eq!(fila, None);

    db.cerrar().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_guardar_y_resolver_plantillas_globales_y_por_empresa() {
    let Some(url) = url() else { return };
    let (db, _pool) = preparar(&url).await;
    let plantillas = notifications_service::plantillas_db::Plantillas::nueva(Arc::clone(&db));
    let empresa = Uuid::new_v4().to_string();
    plantillas
        .guardar(
            None,
            "payment.procesado",
            "email",
            "Pago {{pedido}}",
            "Global {{total}}",
            true,
        )
        .await
        .expect("global");
    let global = plantillas
        .obtener(Some(&empresa), "payment.procesado", "email")
        .await
        .expect("obtener")
        .expect("global");
    assert_eq!(global.cuerpo, "Global {{total}}");
    plantillas
        .guardar(
            Some(&empresa),
            "payment.procesado",
            "email",
            "Pago tenant",
            "Tenant {{total}}",
            true,
        )
        .await
        .expect("tenant");
    let tenant = plantillas
        .obtener(Some(&empresa), "payment.procesado", "email")
        .await
        .expect("obtener")
        .expect("tenant");
    assert_eq!(tenant.cuerpo, "Tenant {{total}}");
    assert_eq!(
        plantillas
            .listar(Some(&empresa))
            .await
            .expect("listar")
            .as_array()
            .expect("array")
            .len(),
        2
    );
}

async fn enviar_admin(
    app: &Router,
    metodo: &str,
    ruta: &str,
    cuerpo: Option<Value>,
    headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut constructor = Request::builder().method(metodo).uri(ruta);
    for (clave, valor) in headers {
        constructor = constructor.header(*clave, *valor);
    }
    let request = constructor
        .header("content-type", "application/json")
        .body(Body::from(
            cuerpo.map(|valor| valor.to_string()).unwrap_or_default(),
        ))
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("cuerpo")
        .to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn admin<'a>(rol: &'a str, tenant: Option<&'a str>) -> Vec<(&'a str, &'a str)> {
    let mut headers = vec![("x-user-id", "admin-1"), ("x-user-rol", rol)];
    if let Some(tenant) = tenant {
        headers.push(("x-tenant", tenant));
    }
    headers
}

#[tokio::test]
async fn debe_exponer_la_api_admin_de_plantillas() {
    let Some(url) = url() else { return };
    let (db, _pool) = preparar(&url).await;
    let plantillas = Arc::new(Plantillas::nueva(Arc::clone(&db)));
    let config = Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config");
    let estado = AppState {
        config: Arc::new(config),
        metrics: Arc::new(Metrics::nueva("notifications-service")),
        db: Some(Arc::clone(&db)),
        plantillas: Some(Arc::clone(&plantillas)),
    };
    let app = build_router(estado);
    let evento = format!("evento.{}", Uuid::new_v4());
    let empresa = Uuid::new_v4().to_string();
    let ruta = format!("/api/v2/admin/notifications/plantillas/{evento}/email");

    let (status, _) = enviar_admin(
        &app,
        "GET",
        "/api/v2/admin/notifications/plantillas",
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = enviar_admin(
        &app,
        "GET",
        "/api/v2/admin/notifications/plantillas",
        None,
        &admin("comprador", None),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, cuerpo) = enviar_admin(
        &app,
        "PUT",
        &ruta,
        Some(json!({ "cuerpo": "   " })),
        &admin("admin", Some(&empresa)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(cuerpo.is_null());

    let (status, cuerpo) = enviar_admin(
        &app,
        "PUT",
        &ruta,
        Some(json!({ "asunto": "Pedido {{pedido}}", "cuerpo": "Hola {{cliente}}" })),
        &admin("admin", Some(&empresa)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cuerpo["data"]["evento"], evento);
    assert_eq!(cuerpo["data"]["empresa_id"], empresa);
    assert_eq!(cuerpo["data"]["canal"], "email");

    let (status, listado) = enviar_admin(
        &app,
        "GET",
        "/api/v2/admin/notifications/plantillas",
        None,
        &admin("admin", Some(&empresa)),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let datos = listado["data"]["plantillas"].as_array().expect("array");
    assert!(datos
        .iter()
        .any(|fila| fila["evento"] == evento && fila["empresa_id"] == empresa));

    let (status, _) = enviar_admin(
        &app,
        "PUT",
        &format!("/api/v2/admin/notifications/plantillas/{evento}/fax"),
        Some(json!({ "cuerpo": "Hola" })),
        &admin("admin", Some(&empresa)),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let estado_sin_plantillas = AppState {
        config: Arc::new(Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config")),
        metrics: Arc::new(Metrics::nueva("notifications-service")),
        db: Some(Arc::clone(&db)),
        plantillas: None,
    };
    let (status, _) = enviar_admin(
        &build_router(estado_sin_plantillas),
        "GET",
        "/api/v2/admin/notifications/plantillas",
        None,
        &admin("admin", None),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    db.cerrar().await;
}
