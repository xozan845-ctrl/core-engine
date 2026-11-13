//! Pruebas PostgreSQL de auth, sesiones y restablecimiento.
//!
//! Solo corren con `TEST_DATABASE_URL`; provisionan el esquema `identity`
//! aplicando las migraciones reales de `infra/db/init` (sin dumps paralelos).

use identity_service::auth::{verificar_token, Sesion};
use identity_service::config::Config;
use identity_service::db::Db;
use identity_service::dominio::auth::Auth;
use identity_service::dominio::password_reset::PasswordReset;
use identity_service::dominio::sesiones::Sesiones;
use identity_service::dominio::usuarios::{CrearUsuario, FiltroUsuarios, Usuarios};
use identity_service::email::{Email, EmailMessage};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

static DB_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());
static PROVISION: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

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

impl EmailFake {
    fn ultimo(&self) -> Option<EmailMessage> {
        self.enviados.lock().expect("email").last().cloned()
    }
}

async fn provisionar(url: &str) {
    let pool = sqlx::PgPool::connect(url).await.expect("postgres");
    // Las migraciones están pensadas para una BD vacía; se reinician los
    // esquemas para que un rerun local sea determinista (nunca contra una BD
    // con datos: el runner usa una BD efímera).
    sqlx::raw_sql(
        "DROP SCHEMA IF EXISTS identity, catalog, stores, orders, logistics,
             commissions, finance, scheduling, field, intelligence, storage,
             notifications, auth CASCADE",
    )
    .execute(&pool)
    .await
    .expect("reset de esquemas de prueba");
    // Se aplican las migraciones reales en el mismo orden que Docker
    // (`docker-entrypoint-initdb.d`), no un dump paralelo: así una deriva entre
    // migraciones y código hace fallar la prueba (como el caso items_vista).
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
            .unwrap_or_else(|error| {
                panic!("migracion {}: {error}", archivo.display());
            });
    }
    pool.close().await;
}

struct Contexto {
    _guard: tokio::sync::MutexGuard<'static, ()>,
    auth: Auth,
    usuarios: Arc<Usuarios>,
    reset: PasswordReset,
    email: Arc<EmailFake>,
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
    let auth = Auth::nueva(Arc::clone(&usuarios), Arc::clone(&sesiones), &config);
    let email = Arc::new(EmailFake::default());
    let reset = PasswordReset::nueva(
        Arc::clone(&db),
        Arc::clone(&usuarios),
        Arc::clone(&sesiones),
        email.clone(),
        "http://app.test".to_string(),
    );
    Some(Contexto {
        _guard: guard,
        auth,
        usuarios,
        reset,
        email,
    })
}

fn datos(correo: &str) -> CrearUsuario {
    CrearUsuario {
        nombre: "Ana Pérez".into(),
        correo: correo.into(),
        contrasena: "secreto123".into(),
        rol: "comprador".into(),
        tenant_id: None,
        personal_id: None,
    }
}

fn correo() -> String {
    format!("ana-{}@tienda.test", Uuid::new_v4())
}

#[tokio::test]
async fn debe_registrar_login_refrescar_y_revocar_la_sesion() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let correo = correo();
    let sesion = ctx.auth.registrar(datos(&correo)).await.expect("registro");
    let payload = verificar_token(&sesion.access_token, "secreto-de-prueba").expect("token");
    assert_eq!(payload.tipo, "access");
    assert_eq!(payload.rol, "comprador");
    assert_eq!(payload.email, correo);
    assert_eq!(sesion.expira_en, 900);

    let login = ctx.auth.login(&correo, "secreto123").await.expect("login");
    assert_eq!(login.usuario.id, sesion.usuario.id);

    let refrescada = ctx
        .auth
        .refrescar(&login.refresh_token)
        .await
        .expect("refresh");
    assert_eq!(refrescada.usuario.id, sesion.usuario.id);
    let payload = verificar_token(&refrescada.refresh_token, "secreto-de-prueba").expect("jwt");
    assert_eq!(payload.tipo, "refresh");
    assert!(payload.jti.is_some());

    let revocadas = ctx
        .auth
        .cerrar_sesion(&sesion.usuario.id, Some(&refrescada.refresh_token))
        .await
        .expect("logout");
    assert_eq!(revocadas, 1);
    let error = ctx
        .auth
        .refrescar(&refrescada.refresh_token)
        .await
        .expect_err("revocada");
    assert_eq!(error.codigo, "NO_AUTORIZADO");
}

#[tokio::test]
async fn debe_rechazar_credenciales_y_correos_duplicados() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let correo = correo();
    ctx.auth.registrar(datos(&correo)).await.expect("registro");

    let error = ctx
        .auth
        .login(&correo, "incorrecta")
        .await
        .expect_err("credenciales");
    assert_eq!(error.status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(error.codigo, "NO_AUTORIZADO");

    let error = ctx
        .usuarios
        .crear(datos(&correo))
        .await
        .expect_err("duplicado");
    assert_eq!(error.status, axum::http::StatusCode::CONFLICT);
}

#[tokio::test]
async fn debe_cambiar_la_contrasena_y_revocar_las_sesiones() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let correo = correo();
    let sesion = ctx.auth.registrar(datos(&correo)).await.expect("registro");

    let error = ctx
        .usuarios
        .cambiar_contrasena(&sesion.usuario.id, "incorrecta", "nueva1234")
        .await
        .expect_err("actual");
    assert_eq!(error.codigo, "NO_AUTORIZADO");

    ctx.usuarios
        .cambiar_contrasena(&sesion.usuario.id, "secreto123", "nueva1234")
        .await
        .expect("cambio");
    assert!(ctx.auth.login(&correo, "secreto123").await.is_err());
    assert!(ctx.auth.login(&correo, "nueva1234").await.is_ok());
}

#[tokio::test]
async fn debe_restablecer_la_contrasena_con_un_token_de_un_solo_uso() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let correo = correo();
    ctx.auth.registrar(datos(&correo)).await.expect("registro");

    ctx.reset.solicitar(&correo).await.expect("solicitud");
    let mensaje = ctx.email.ultimo().expect("correo");
    let token = mensaje
        .text
        .split("token=")
        .nth(1)
        .and_then(|resto| resto.split_whitespace().next())
        .expect("token")
        .to_string();

    ctx.reset
        .confirmar(&token, "reset1234")
        .await
        .expect("confirmar");
    assert!(ctx.auth.login(&correo, "reset1234").await.is_ok());

    let error = ctx
        .reset
        .confirmar(&token, "otra1234")
        .await
        .expect_err("usado");
    assert_eq!(error.status, axum::http::StatusCode::GONE);
    assert_eq!(error.codigo, "TOKEN_USADO");
}

#[tokio::test]
async fn debe_no_revelar_correos_inexistentes_en_el_reset() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    ctx.reset
        .solicitar("nadie@tienda.test")
        .await
        .expect("solicitud anonima");
    assert!(ctx.email.ultimo().is_none());
}

#[tokio::test]
async fn debe_listar_con_filtros_y_sanear_el_hash() {
    let Some(ctx) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite");
        return;
    };
    let correo = correo();
    let sesion = ctx.auth.registrar(datos(&correo)).await.expect("registro");
    let listado = ctx
        .usuarios
        .listar(FiltroUsuarios {
            rol: Some("comprador".into()),
            tenant_id: None,
        })
        .await
        .expect("listar");
    let creado = listado
        .iter()
        .find(|usuario| usuario.id == sesion.usuario.id)
        .expect("usuario listado");
    assert_eq!(creado.correo, correo);
    assert!(serde_json::to_value(creado)
        .expect("json")
        .get("contrasena_hash")
        .is_none());

    let total = ctx.usuarios.contar_vendedores().await.expect("contar");
    assert!(total >= 0);
}

fn _assert_sesion_serializa(_: &Sesion) {}
