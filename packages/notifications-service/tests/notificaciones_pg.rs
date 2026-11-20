//! Pruebas de integración contra PostgreSQL (solo si `TEST_DATABASE_URL` está
//! definida y el esquema `notifications` existe o puede crearse). El runner de
//! CI las ejecuta con la BD efímera.

use async_trait::async_trait;
use axum::body::{Body, Bytes};
use axum::http::{HeaderMap, Request, StatusCode};
use axum::routing::post;
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
use notifications_service::webhooks::{firma_de, Webhooks};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex as StdMutex;
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
    sqlx::raw_sql(include_str!(
        "../../../infra/db/init/99_notificaciones_webhooks.sql"
    ))
    .execute(&pool)
    .await
    .expect("migracion webhooks");
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
    let mut headers = vec![
        ("x-user-id", "11111111-1111-4111-8111-111111111111"),
        ("x-user-rol", rol),
    ];
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
        webhooks: None,
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
        webhooks: None,
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

struct RegistroWebhook {
    headers: HeaderMap,
    cuerpo: Vec<u8>,
}

struct ServidorWebhook {
    url: String,
    recibidos: Arc<StdMutex<Vec<RegistroWebhook>>>,
}

async fn servidor_webhook(respuestas: Vec<StatusCode>) -> ServidorWebhook {
    let recibidos = Arc::new(StdMutex::new(Vec::new()));
    let estado = Arc::clone(&recibidos);
    let contador = Arc::new(AtomicUsize::new(0));
    let app = Router::new().route(
        "/hook",
        post(move |headers: HeaderMap, cuerpo: Bytes| {
            let estado = Arc::clone(&estado);
            let contador = Arc::clone(&contador);
            let respuestas = respuestas.clone();
            async move {
                let indice = contador.fetch_add(1, Ordering::SeqCst);
                estado.lock().expect("recibidos").push(RegistroWebhook {
                    headers,
                    cuerpo: cuerpo.to_vec(),
                });
                respuestas
                    .get(indice)
                    .copied()
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let url = format!("http://{}/hook", listener.local_addr().expect("addr"));
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    ServidorWebhook { url, recibidos }
}

#[tokio::test]
async fn debe_entregar_y_firmar_un_webhook_sin_duplicar() {
    let Some(url) = url() else { return };
    let (db, pool) = preparar(&url).await;
    let servidor = servidor_webhook(vec![StatusCode::OK]).await;
    let webhooks = Webhooks::nueva(Arc::clone(&db), 2000, false);
    let empresa = Uuid::new_v4().to_string();
    let creado = webhooks
        .crear(
            &empresa,
            &servidor.url,
            "ERP del piloto",
            vec!["order.created".into()],
            &Uuid::new_v4().to_string(),
        )
        .await
        .expect("crear webhook");
    let secreto = creado["secreto"].as_str().expect("secreto").to_string();
    let event_id = Uuid::new_v4();
    let evento = serde_json::json!({
        "event_id": event_id,
        "tipo": "order.created",
        "data": { "empresa_id": empresa, "order_id": Uuid::new_v4() }
    });

    webhooks.despachar(&evento).await.expect("despachar");
    webhooks
        .despachar(&evento)
        .await
        .expect("despachar de nuevo");

    {
        let registros = servidor.recibidos.lock().expect("recibidos");
        assert_eq!(registros.len(), 1);
        let cabeceras = &registros[0].headers;
        assert_eq!(cabeceras["x-core-event"], "order.created");
        assert_eq!(
            cabeceras["x-core-signature"],
            firma_de(&secreto, &registros[0].cuerpo)
        );
        let payload: Value = serde_json::from_slice(&registros[0].cuerpo).expect("json");
        assert_eq!(payload["event_id"], event_id.to_string());
    }

    let estado: String =
        sqlx::query_scalar("SELECT estado FROM notifications.webhook_entregas WHERE event_id = $1")
            .bind(event_id)
            .fetch_one(&pool)
            .await
            .expect("estado");
    assert_eq!(estado, "entregado");
    db.cerrar().await;
}

#[tokio::test]
async fn debe_reintentar_una_entrega_fallida() {
    let Some(url) = url() else { return };
    let (db, pool) = preparar(&url).await;
    let servidor = servidor_webhook(vec![StatusCode::INTERNAL_SERVER_ERROR, StatusCode::OK]).await;
    let webhooks = Webhooks::nueva(Arc::clone(&db), 2000, false);
    let empresa = Uuid::new_v4().to_string();
    webhooks
        .crear(
            &empresa,
            &servidor.url,
            "Reintentos",
            vec![],
            &Uuid::new_v4().to_string(),
        )
        .await
        .expect("crear webhook");
    let event_id = Uuid::new_v4();
    webhooks
        .despachar(&serde_json::json!({
            "event_id": event_id,
            "tipo": "payment.procesado",
            "data": { "empresa_id": empresa }
        }))
        .await
        .expect("despachar");

    let (entrega_id, estado, intentos): (Uuid, String, i32) = sqlx::query_as(
        "SELECT id, estado, intentos FROM notifications.webhook_entregas WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&pool)
    .await
    .expect("entrega");
    assert_eq!(estado, "fallido");
    assert_eq!(intentos, 1);

    webhooks
        .reintentar_entrega(&entrega_id.to_string(), &empresa)
        .await
        .expect("reintentar");
    let (estado, intentos): (String, i32) =
        sqlx::query_as("SELECT estado, intentos FROM notifications.webhook_entregas WHERE id = $1")
            .bind(entrega_id)
            .fetch_one(&pool)
            .await
            .expect("entrega reintentada");
    assert_eq!(estado, "entregado");
    assert_eq!(intentos, 2);
    assert_eq!(servidor.recibidos.lock().expect("recibidos").len(), 2);
    drop(servidor);
    db.cerrar().await;
}

#[tokio::test]
async fn debe_gestionar_webhooks_por_la_api_admin() {
    let Some(url) = url() else { return };
    let (db, _pool) = preparar(&url).await;
    let servidor = servidor_webhook(vec![StatusCode::OK]).await;
    let webhooks = Arc::new(Webhooks::nueva(Arc::clone(&db), 2000, false));
    let estado = AppState {
        config: Arc::new(Config::from_pairs(Vec::<(&str, &str)>::new()).expect("config")),
        metrics: Arc::new(Metrics::nueva("notifications-service")),
        db: Some(Arc::clone(&db)),
        plantillas: None,
        webhooks: Some(webhooks),
    };
    let app = build_router(estado);
    let empresa = Uuid::new_v4().to_string();
    let headers = admin("admin", Some(&empresa));

    let (status, creado) = enviar_admin(
        &app,
        "POST",
        "/api/v2/admin/notifications/webhooks",
        Some(json!({
            "url": servidor.url,
            "descripcion": "ERP",
            "eventos": ["order.created", "payment.procesado"]
        })),
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(creado["data"]["secreto"].is_string());
    let id = creado["data"]["id"].as_str().expect("id").to_string();

    let (status, listado) = enviar_admin(
        &app,
        "GET",
        "/api/v2/admin/notifications/webhooks",
        None,
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listado["data"]["webhooks"][0]["descripcion"], "ERP");
    assert!(listado["data"]["webhooks"][0].get("secreto").is_none());

    let (status, rotado) = enviar_admin(
        &app,
        "POST",
        &format!("/api/v2/admin/notifications/webhooks/{id}/rotar-secreto"),
        None,
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(rotado["data"]["secreto"].is_string());

    let (status, actualizado) = enviar_admin(
        &app,
        "PATCH",
        &format!("/api/v2/admin/notifications/webhooks/{id}"),
        Some(json!({ "activo": false })),
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(actualizado["data"]["activo"], false);

    let (status, _) = enviar_admin(
        &app,
        "DELETE",
        &format!("/api/v2/admin/notifications/webhooks/{id}"),
        None,
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    db.cerrar().await;
}

#[tokio::test]
async fn debe_exportar_y_purgar_plantillas_y_webhooks_del_tenant() {
    let Some(url) = url() else { return };
    let (db, pool) = preparar(&url).await;
    let empresa = Uuid::new_v4().to_string();
    let plantillas = Arc::new(Plantillas::nueva(Arc::clone(&db)));
    plantillas
        .guardar(
            Some(&empresa),
            "order.created",
            "email",
            "Pedido",
            "Hola {{cliente}}",
            true,
        )
        .await
        .expect("plantilla");
    let webhooks = Arc::new(Webhooks::nueva(Arc::clone(&db), 2000, false));
    webhooks
        .crear(
            &empresa,
            "https://erp.example.com/hook",
            "ERP",
            vec!["order.created".into()],
            &Uuid::new_v4().to_string(),
        )
        .await
        .expect("webhook");
    let estado = AppState {
        config: Arc::new(
            Config::from_pairs([("INTERNAL_API_KEY", "clave-interna")]).expect("config"),
        ),
        metrics: Arc::new(Metrics::nueva("notifications-service")),
        db: Some(Arc::clone(&db)),
        plantillas: Some(Arc::clone(&plantillas)),
        webhooks: Some(Arc::clone(&webhooks)),
    };
    let app = build_router(estado);
    let headers = vec![("x-internal-key", "clave-interna")];

    let (status, exportado) = enviar_admin(
        &app,
        "POST",
        "/internal/notifications/tenant/exportar",
        Some(json!({ "empresa_id": empresa })),
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        exportado["datos"]["plantillas"]
            .as_array()
            .expect("plantillas")
            .len(),
        1
    );
    assert_eq!(
        exportado["datos"]["webhooks"]["webhooks"]
            .as_array()
            .expect("webhooks")
            .len(),
        1
    );

    let (status, purgado) = enviar_admin(
        &app,
        "POST",
        "/internal/notifications/tenant/purgar",
        Some(json!({ "empresa_id": empresa })),
        &headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(purgado["detalle"]["plantillas"], 1);
    assert_eq!(purgado["detalle"]["webhooks"], 1);

    let plantillas_restantes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications.plantillas WHERE empresa_id = $1::uuid",
    )
    .bind(&empresa)
    .fetch_one(&pool)
    .await
    .expect("plantillas restantes");
    let webhooks_restantes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications.webhooks WHERE empresa_id = $1::uuid",
    )
    .bind(&empresa)
    .fetch_one(&pool)
    .await
    .expect("webhooks restantes");
    assert_eq!(plantillas_restantes, 0);
    assert_eq!(webhooks_restantes, 0);
    db.cerrar().await;
}
