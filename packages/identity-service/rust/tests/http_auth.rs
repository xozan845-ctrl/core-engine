//! Pruebas HTTP de `/api/v2/auth` con PostgreSQL real.
//!
//! Cubren el contrato observable: envelope, códigos, validación y guards.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use identity_service::config::Config;
use identity_service::db::Db;
use identity_service::dominio::auth::Auth;
use identity_service::dominio::password_reset::PasswordReset;
use identity_service::dominio::sesiones::Sesiones;
use identity_service::dominio::usuarios::Usuarios;
use identity_service::email::{Email, EmailMessage};
use identity_service::http::build_router;
use identity_service::metrics::Metrics;
use identity_service::state::{AppState, Servicios};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

static DB_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static PROVISION: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

#[derive(Default)]
struct EmailFake {
    enviados: Mutex<Vec<EmailMessage>>,
}

impl Email for EmailFake {
    fn enviar(&self, mensaje: &EmailMessage) -> Result<(), String> {
        self.enviados.lock().expect("email").push(mensaje.clone());
        Ok(())
    }
}

struct Contexto {
    _guard: tokio::sync::MutexGuard<'static, ()>,
    app: Router,
    usuarios: Arc<Usuarios>,
    email: Arc<EmailFake>,
}

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

async fn provisionar(url: &str) {
    let pool = sqlx::PgPool::connect(url).await.expect("postgres");
    sqlx::raw_sql(
        "DROP SCHEMA IF EXISTS identity, catalog, stores, orders, logistics,
             commissions, finance, scheduling, field, intelligence, storage,
             notifications, auth CASCADE",
    )
    .execute(&pool)
    .await
    .expect("reset de esquemas de prueba");
    let mut archivos: Vec<std::path::PathBuf> = std::fs::read_dir(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../infra/db/init"
    ))
    .expect("infra/db/init")
    .filter_map(|entrada| entrada.ok().map(|entrada| entrada.path()))
    .filter(|ruta| ruta.extension().is_some_and(|extension| extension == "sql"))
    .collect();
    archivos.sort();
    for archivo in archivos {
        let sql = std::fs::read_to_string(&archivo).expect("leer migracion");
        let sql = sql.trim_start_matches('\u{feff}');
        sqlx::raw_sql(sql)
            .execute(&pool)
            .await
            .unwrap_or_else(|error| panic!("migracion {}: {error}", archivo.display()));
    }
    pool.close().await;
}

async fn preparar() -> Option<Contexto> {
    let url = url()?;
    let guard = DB_LOCK.lock().await;
    let url_inicial = url.clone();
    PROVISION
        .get_or_init(|| async move { provisionar(&url_inicial).await })
        .await;
    let db = Arc::new(Db::conectar(&url).await.expect("db"));
    let config = Config::from_pairs([
        ("DATABASE_URL", url.as_str()),
        ("JWT_SECRET", "secreto-de-prueba"),
        ("EMAIL_MODE", "log"),
    ])
    .expect("config");
    let usuarios = Arc::new(Usuarios::nueva(Arc::clone(&db)));
    let sesiones = Arc::new(Sesiones::nueva(Arc::clone(&db)));
    let auth = Arc::new(Auth::nueva(
        Arc::clone(&usuarios),
        Arc::clone(&sesiones),
        &config,
    ));
    let email = Arc::new(EmailFake::default());
    let password_reset = Arc::new(PasswordReset::nueva(
        Arc::clone(&db),
        Arc::clone(&usuarios),
        Arc::clone(&sesiones),
        email.clone(),
        "http://app.test".to_string(),
    ));
    let metrics = Arc::new(Metrics::nueva("identity-service"));
    let estado = AppState::con_servicios(
        config,
        metrics,
        Servicios {
            auth,
            usuarios: Arc::clone(&usuarios),
            password_reset,
        },
    );
    Some(Contexto {
        _guard: guard,
        app: build_router(estado),
        usuarios,
        email,
    })
}

fn correo() -> String {
    format!("http-{}@tienda.test", Uuid::new_v4())
}

async fn enviar(
    app: &Router,
    metodo: &str,
    ruta: &str,
    cuerpo: Value,
    headers: &[(&str, &str)],
) -> (StatusCode, Value) {
    let mut constructor = Request::builder().method(metodo).uri(ruta);
    for (clave, valor) in headers {
        constructor = constructor.header(*clave, *valor);
    }
    let request = constructor
        .header("content-type", "application/json")
        .body(Body::from(cuerpo.to_string()))
        .expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("cuerpo")
        .to_bytes();
    let valor = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, valor)
}

fn autorizacion(id: &str, rol: &str) -> Vec<(&'static str, String)> {
    vec![
        ("x-user-id", id.to_string()),
        ("x-user-rol", rol.to_string()),
        ("x-user-email", "actor@test.dev".to_string()),
    ]
}

fn headers<'a>(pares: &'a [(&'static str, String)]) -> Vec<(&'static str, &'a str)> {
    pares
        .iter()
        .map(|(clave, valor)| (*clave, valor.as_str()))
        .collect()
}

#[tokio::test]
async fn debe_registrar_login_me_refresh_y_logout() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let correo = correo();
    let (status, registro) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/registro",
        json!({
            "nombre": "Ana Pérez",
            "correo": correo,
            "contrasena": "secreto123",
            "rol": "comprador"
        }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(registro["data"]["usuario"]["correo"], correo);
    assert_eq!(registro["data"]["expira_en"], 900);
    assert_eq!(registro["meta"]["statusCode"], 201);
    let id = registro["data"]["usuario"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    let refresh = registro["data"]["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_string();

    let (status, login) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/login",
        json!({ "correo": correo, "contrasena": "secreto123" }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(login["data"]["usuario"]["id"], id);

    let auth = autorizacion(&id, "comprador");
    let (status, me) = enviar(
        &ctx.app,
        "GET",
        "/api/v2/auth/me",
        Value::Null,
        &headers(&auth),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["data"]["user_id"], id);
    assert_eq!(me["data"]["rol"], "comprador");

    let (status, refrescada) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/refresh",
        json!({ "refresh_token": refresh }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let nuevo_refresh = refrescada["data"]["refresh_token"]
        .as_str()
        .expect("refresh")
        .to_string();

    let (status, logout) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/logout",
        json!({ "refresh_token": nuevo_refresh }),
        &headers(&auth),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(logout["data"]["ok"], true);

    let (status, error) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/refresh",
        json!({ "refresh_token": nuevo_refresh }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error["codigo"], "NO_AUTORIZADO");
}

#[tokio::test]
async fn debe_validar_el_cuerpo_y_los_guards() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let (status, error) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/registro",
        json!({ "nombre": "x" }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "VALIDACION");
    assert_eq!(error["detalles"][0]["campo"], "nombre");

    let id = Uuid::new_v4().to_string();
    let lider = autorizacion(&id, "comprador");
    let (status, error) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/crear-usuario",
        json!({
            "nombre": "Otro",
            "correo": correo(),
            "contrasena": "secreto123",
            "rol": "admin"
        }),
        &headers(&lider),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["codigo"], "ACCESO_DENEGADO");

    let admin = autorizacion(&Uuid::new_v4().to_string(), "admin");
    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/crear-usuario",
        json!({
            "nombre": "Nuevo",
            "correo": correo(),
            "contrasena": "secreto123",
            "rol": "logistica"
        }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn debe_crear_usuario_como_admin_y_cambiar_contrasena() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let admin = autorizacion(&Uuid::new_v4().to_string(), "super_admin");
    let correo = correo();
    let (status, creado) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/crear-usuario",
        json!({
            "nombre": "Nuevo Usuario",
            "correo": correo,
            "contrasena": "secreto123",
            "rol": "operativo",
            "tenant_id": Uuid::new_v4().to_string()
        }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = creado["data"]["id"].as_str().expect("id").to_string();
    assert_eq!(creado["data"]["rol"], "operativo");

    let auth = autorizacion(&id, "operativo");
    let (status, error) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/cambiar-contrasena",
        json!({ "actual": "incorrecta", "nueva": "nueva1234" }),
        &headers(&auth),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error["codigo"], "NO_AUTORIZADO");

    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/cambiar-contrasena",
        json!({ "actual": "secreto123", "nueva": "nueva1234" }),
        &headers(&auth),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/login",
        json!({ "correo": correo, "contrasena": "nueva1234" }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn debe_restablecer_con_correo_y_confirmar_el_token() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let correo = correo();
    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/registro",
        json!({
            "nombre": "Ana",
            "correo": correo,
            "contrasena": "secreto123",
            "rol": "comprador"
        }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, respuesta) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/restablecer-contrasena",
        json!({ "correo": correo }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(respuesta["data"]["ok"], true);

    let mensaje = ctx
        .email
        .enviados
        .lock()
        .expect("email")
        .last()
        .cloned()
        .expect("correo");
    let token = mensaje
        .text
        .split("token=")
        .nth(1)
        .and_then(|resto| resto.split_whitespace().next())
        .expect("token")
        .to_string();

    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/restablecer-contrasena/confirmar",
        json!({ "token": token, "contrasena": "reset1234" }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/restablecer-contrasena/confirmar",
        json!({ "token": token, "contrasena": "otra1234" }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::GONE);
}

#[tokio::test]
async fn debe_vincular_personal_y_actualizar_el_nombre() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let correo = correo();
    let (_, registro) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/registro",
        json!({
            "nombre": "Ana",
            "correo": correo,
            "contrasena": "secreto123",
            "rol": "comprador"
        }),
        &[],
    )
    .await;
    let id = registro["data"]["usuario"]["id"]
        .as_str()
        .expect("id")
        .to_string();
    let auth = autorizacion(&id, "comprador");
    let personal = Uuid::new_v4().to_string();
    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/vincular-personal",
        json!({ "personal_id": personal, "nombre": "Ana María" }),
        &headers(&auth),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let usuario = ctx
        .usuarios
        .encontrar_por_id(&id)
        .await
        .expect("usuario")
        .expect("existe");
    assert_eq!(usuario.personal_id.as_deref(), Some(personal.as_str()));
    assert_eq!(usuario.nombre, "Ana María");
}

#[tokio::test]
async fn debe_rechazar_me_sin_contexto() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let (status, error) = enviar(&ctx.app, "GET", "/api/v2/auth/me", Value::Null, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(error["codigo"], "NO_AUTORIZADO");
}
