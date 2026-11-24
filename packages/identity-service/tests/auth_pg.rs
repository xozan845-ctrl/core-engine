//! Pruebas PostgreSQL de auth, sesiones y restablecimiento.
//!
//! Solo corren con `TEST_DATABASE_URL`; provisionan el esquema `identity`
//! aplicando las migraciones reales de `infra/db/init` (sin dumps paralelos).

use identity_service::auth::{verificar_token, Sesion, UsuarioContexto};
use identity_service::config::Config;
use identity_service::crypto::CifradoSecreto;
use identity_service::db::Db;
use identity_service::dominio::auth::Auth;
use identity_service::dominio::password_reset::PasswordReset;
use identity_service::dominio::regiones::Regiones;
use identity_service::dominio::sesiones::Sesiones;
use identity_service::dominio::usuarios::{CrearUsuario, FiltroUsuarios, Usuarios};
use identity_service::email::{Email, EmailMessage};
use identity_service::storage::Storage;
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
             notifications, auth, inventory, pricing, item, analytics CASCADE",
    )
    .execute(&pool)
    .await
    .expect("reset de esquemas de prueba");
    // Se aplican las migraciones reales en el mismo orden que Docker
    // (`docker-entrypoint-initdb.d`), no un dump paralelo: así una deriva entre
    // migraciones y código hace fallar la prueba (como el caso items_vista).
    let mut archivos: Vec<std::path::PathBuf> =
        std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../infra/db/init"))
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
    db: Arc<Db>,
    sesiones: Arc<Sesiones>,
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
    let storage = Arc::new(Storage::nueva("http://storage.test", None));
    let cifrado =
        Arc::new(CifradoSecreto::nueva(config.clave_mfa().expect("clave mfa")).expect("cifrado"));
    let usuarios = Arc::new(Usuarios::nueva(
        Arc::clone(&db),
        storage,
        Arc::clone(&cifrado),
    ));
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
        db: Arc::clone(&db),
        sesiones,
    })
}

fn datos(correo: &str) -> CrearUsuario {
    CrearUsuario {
        nombre: "Ana Pérez".into(),
        correo: correo.into(),
        contrasena: "ClaveSegura2026!".into(),
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

    let login = ctx
        .auth
        .login(&correo, "ClaveSegura2026!")
        .await
        .expect("login");
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
        .cambiar_contrasena(&sesion.usuario.id, "incorrecta", "NuevaClave2026!")
        .await
        .expect_err("actual");
    assert_eq!(error.codigo, "NO_AUTORIZADO");

    ctx.usuarios
        .cambiar_contrasena(&sesion.usuario.id, "ClaveSegura2026!", "NuevaClave2026!")
        .await
        .expect("cambio");
    assert!(ctx.auth.login(&correo, "ClaveSegura2026!").await.is_err());
    assert!(ctx.auth.login(&correo, "NuevaClave2026!").await.is_ok());
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
        .confirmar(&token, "ResetClave2026!")
        .await
        .expect("confirmar");
    assert!(ctx.auth.login(&correo, "ResetClave2026!").await.is_ok());

    let error = ctx
        .reset
        .confirmar(&token, "OtraClave2026!")
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

#[tokio::test]
async fn debe_cifrar_el_secreto_mfa_y_permitir_reset_administrativo() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let usuario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "MFA cifrado".into(),
            correo: correo(),
            contrasena: "ClaveSegura2026!".into(),
            rol: "comprador".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("usuario");
    let iniciado = ctx
        .usuarios
        .iniciar_mfa(&usuario.id)
        .await
        .expect("iniciar mfa");
    let secreto = iniciado["secreto"].as_str().expect("secreto").to_string();
    let almacenado: String =
        sqlx::query_scalar("SELECT mfa_secreto FROM identity.usuarios WHERE id = $1::uuid")
            .bind(&usuario.id)
            .fetch_one(ctx.db.pool())
            .await
            .expect("secreto");
    assert!(almacenado.starts_with("v1:"));
    assert_ne!(almacenado, secreto);
    assert!(!almacenado.contains(&secreto));

    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("reloj")
        .as_secs();
    let codigo = identity_service::dominio::totp::codigo(&secreto, ahora).expect("codigo");
    ctx.usuarios
        .confirmar_mfa(&usuario.id, &codigo)
        .await
        .expect("confirmar mfa");
    let habilitado: bool =
        sqlx::query_scalar("SELECT mfa_habilitado FROM identity.usuarios WHERE id = $1::uuid")
            .bind(&usuario.id)
            .fetch_one(ctx.db.pool())
            .await
            .expect("habilitado");
    assert!(habilitado);

    ctx.usuarios.reset_mfa(&usuario.id).await.expect("reset");
    let estado: (Option<String>, bool) = sqlx::query_as(
        "SELECT mfa_secreto, mfa_habilitado FROM identity.usuarios WHERE id = $1::uuid",
    )
    .bind(&usuario.id)
    .fetch_one(ctx.db.pool())
    .await
    .expect("estado");
    assert_eq!(estado.0, None);
    assert!(!estado.1);
    let codigos: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM identity.mfa_codigos_recuperacion WHERE usuario_id = $1::uuid",
    )
    .bind(&usuario.id)
    .fetch_one(ctx.db.pool())
    .await
    .expect("codigos");
    assert_eq!(codigos, 0);
}

#[tokio::test]
async fn debe_registrar_la_ip_de_la_sesion() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let usuario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Sesion IP".into(),
            correo: correo(),
            contrasena: "ClaveSegura2026!".into(),
            rol: "comprador".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("usuario");
    let jti = Uuid::new_v4().to_string();
    ctx.sesiones
        .crear(
            &usuario.id,
            &jti,
            chrono::Utc::now() + chrono::Duration::seconds(600),
            None,
            Some("agente-test"),
            Some("203.0.113.7"),
        )
        .await
        .expect("sesion");
    let sesiones = ctx
        .sesiones
        .listar_activas(&usuario.id)
        .await
        .expect("listar");
    let sesion = sesiones
        .iter()
        .find(|valor| valor["user_agent"] == "agente-test")
        .expect("sesion");
    assert_eq!(sesion["ip"], "203.0.113.7");
}

#[tokio::test]
async fn debe_guardar_y_validar_el_telefono_del_usuario() {
    let Some(ctx) = preparar().await else { return };
    let usuario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Telefono".into(),
            correo: correo(),
            contrasena: "ClaveSegura2026!".into(),
            rol: "comprador".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("usuario");
    ctx.usuarios
        .actualizar_telefono(&usuario.id, Some("+505 8888-7777"))
        .await
        .expect("telefono");
    let guardado = ctx
        .usuarios
        .encontrar_por_id(&usuario.id)
        .await
        .expect("buscar")
        .expect("usuario");
    assert_eq!(guardado.telefono.as_deref(), Some("+505 8888-7777"));
    let error = ctx
        .usuarios
        .actualizar_telefono(&usuario.id, Some("abc"))
        .await
        .expect_err("invalido");
    assert_eq!(error.codigo, "TELEFONO_INVALIDO");
}

#[tokio::test]
async fn debe_seleccionar_y_cambiar_la_region_tributaria_del_tenant() {
    let Some(ctx) = preparar().await else { return };
    let regiones = Regiones::nueva(Arc::clone(&ctx.db));

    let disponibles = regiones
        .listar(Some("pack_retail"), Some(1))
        .await
        .expect("regiones del pack");
    let nicaragua = disponibles
        .iter()
        .find(|region| region.codigo == "NI")
        .expect("Nicaragua disponible");
    assert!(nicaragua.predeterminada);
    assert_eq!(nicaragua.moneda_iso, "NIO");
    assert_eq!(nicaragua.zona_horaria, "America/Managua");

    let actor_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO identity.usuarios (id, nombre, correo, contrasena_hash, rol)
         VALUES ($1, 'Tester fiscal', $2, 'no-usada', 'super_admin')",
    )
    .bind(actor_id)
    .bind(format!("{actor_id}@regiones.test"))
    .execute(ctx.db.pool())
    .await
    .expect("usuario actor");

    let empresa_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO identity.empresas
           (id, nombre, slug, vertical_id, pack_codigo, pack_version, creado_por,
            region_tributaria, pais, moneda, zona_horaria)
         VALUES ($1, 'Empresa fiscal', $3, 'retail', 'pack_retail', 1, $2, 'NI',
                 'Nicaragua', 'NIO', 'America/Managua')",
    )
    .bind(empresa_id)
    .bind(actor_id)
    .bind(format!(
        "empresa-fiscal-{}",
        &empresa_id.simple().to_string()[..8]
    ))
    .execute(ctx.db.pool())
    .await
    .expect("empresa");

    let actual = regiones
        .de_empresa(&empresa_id.to_string())
        .await
        .expect("region")
        .expect("asignada");
    assert_eq!(actual.region.codigo, "NI");

    let actor = UsuarioContexto {
        user_id: actor_id.to_string(),
        email: format!("{actor_id}@regiones.test"),
        rol: "super_admin".to_string(),
        nombre: None,
        tenant_id: None,
        personal_id: None,
        participacion_porcentaje: None,
        rol_empresa_id: None,
        negocio_id: None,
        permisos: Vec::new(),
    };

    // Una región existente pero no declarada por el pack se rechaza.
    sqlx::query(
        "INSERT INTO finance.jurisdicciones
           (codigo_pais, nombre, pais_iso, moneda, moneda_iso, simbolo_moneda,
            tasa_iva, tasa_ir, locale, zona_horaria, activa)
         VALUES ('US', 'Estados Unidos', 'US', 'US$', 'USD', 'US$',
                 0.0, 0.0, 'en-US', 'America/New_York', true)",
    )
    .execute(ctx.db.pool())
    .await
    .expect("region US");
    let error = regiones
        .asignar(&empresa_id.to_string(), "US", "", &actor)
        .await
        .expect_err("region no declarada");
    assert_eq!(error.codigo, "REGION_NO_DISPONIBLE");

    // Al declararla en el pack, el cambio actualiza perfil e historial.
    sqlx::query(
        "INSERT INTO identity.pack_regiones
           (pack_codigo, pack_version, region_codigo, predeterminada, activo)
         VALUES ('pack_retail', 1, 'US', false, true)",
    )
    .execute(ctx.db.pool())
    .await
    .expect("pack region US");
    let cambiada = regiones
        .asignar(
            &empresa_id.to_string(),
            "US",
            "Prueba de cambio fiscal",
            &actor,
        )
        .await
        .expect("cambio");
    assert_eq!(cambiada.region.codigo, "US");
    assert_eq!(cambiada.region.moneda_iso, "USD");

    let (pais, moneda, zona): (String, String, String) =
        sqlx::query_as("SELECT pais, moneda, zona_horaria FROM identity.empresas WHERE id = $1")
            .bind(empresa_id)
            .fetch_one(ctx.db.pool())
            .await
            .expect("empresa actualizada");
    assert_eq!(pais, "Estados Unidos");
    assert_eq!(moneda, "USD");
    assert_eq!(zona, "America/New_York");

    let historial: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM identity.empresa_region_historial WHERE empresa_id = $1",
    )
    .bind(empresa_id)
    .fetch_one(ctx.db.pool())
    .await
    .expect("historial");
    assert_eq!(historial, 1);
}
