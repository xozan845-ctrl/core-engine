//! Pruebas HTTP de `/api/v2/auth` con PostgreSQL real.
//!
//! Cubren el contrato observable: envelope, códigos, validación y guards.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use identity_service::config::Config;
use identity_service::db::Db;
use identity_service::dominio::api_keys::ApiKeys;
use identity_service::dominio::auth::Auth;
use identity_service::dominio::capacidades::Capacidades;
use identity_service::dominio::configuracion_sitio::ConfiguracionSitio;
use identity_service::dominio::empresas::Empresas;
use identity_service::dominio::extensiones::Extensiones;
use identity_service::dominio::packs::Packs;
use identity_service::dominio::partes::Partes;
use identity_service::dominio::password_reset::PasswordReset;
use identity_service::dominio::personal::Personal;
use identity_service::dominio::registros::Registros;
use identity_service::dominio::sesiones::Sesiones;
use identity_service::dominio::usuarios::{CrearUsuario, Usuarios};
use identity_service::email::{Email, EmailMessage};
use identity_service::http::build_router;
use identity_service::metrics::Metrics;
use identity_service::state::{AppState, Servicios};
use identity_service::storage::Storage;
use identity_service::stores::Stores;
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
    db: Arc<Db>,
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
    let storage = Arc::new(Storage::nueva("http://127.0.0.1:9", None));
    let usuarios = Arc::new(Usuarios::nueva(Arc::clone(&db), Arc::clone(&storage)));
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
    let api_keys = Arc::new(ApiKeys::nueva(Arc::clone(&db), &config));
    let capacidades = Arc::new(Capacidades::nueva(Arc::clone(&db)));
    let personal = Arc::new(Personal::nueva(
        Arc::clone(&db),
        Arc::clone(&auth),
        email.clone(),
        Arc::new(Stores::nueva("http://127.0.0.1:9", None, 1000)),
        "http://app.test".to_string(),
    ));
    let extensiones = Arc::new(Extensiones::nueva(Arc::clone(&db)));
    let partes = Arc::new(Partes::nueva(Arc::clone(&db)));
    let registros = Arc::new(Registros::nueva(Arc::clone(&db), Arc::clone(&capacidades)));
    let metrics = Arc::new(Metrics::nueva("identity-service"));
    let estado = AppState::con_servicios(
        config,
        metrics,
        Servicios {
            auth,
            usuarios: Arc::clone(&usuarios),
            api_keys,
            capacidades: Arc::clone(&capacidades),
            configuracion: Arc::new(ConfiguracionSitio::nueva(
                Arc::clone(&db),
                Arc::clone(&storage),
                None,
            )),
            empresas: Arc::new(Empresas::nueva(
                Arc::clone(&db),
                Arc::clone(&usuarios),
                Arc::clone(&storage),
            )),
            extensiones,
            personal,
            partes,
            registros,
            packs: Arc::new(Packs::nueva(Arc::clone(&db))),
            password_reset,
        },
    );
    Some(Contexto {
        _guard: guard,
        app: build_router(estado),
        usuarios,
        email,
        db,
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

#[tokio::test]
async fn debe_gestionar_packs_y_verticales() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let sufijo = Uuid::new_v4().simple().to_string();
    let sufijo = &sufijo[..8];
    let modelo = format!("modelo_{sufijo}");
    let vertical = format!("vert_{sufijo}");
    let pack = format!("pack_p{sufijo}");
    let admin = autorizacion(&Uuid::new_v4().to_string(), "super_admin");

    let (status, creado) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/packs/modelos-operacion",
        json!({ "codigo": modelo, "nombre": "Modelo de prueba" }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(creado["data"]["codigo"], modelo);

    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/packs/verticales",
        json!({
            "codigo": vertical,
            "nombre": "Vertical de prueba",
            "modelo_operacion": modelo
        }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, pack_creado) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/packs",
        json!({
            "codigo": pack,
            "version": 1,
            "vertical_codigo": vertical,
            "nombre": "Pack de prueba",
            "capacidades": [{ "codigo": "catalogo_items" }]
        }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(pack_creado["data"]["codigo"], pack);
    assert_eq!(
        pack_creado["data"]["capacidades"][0]["capacidad_codigo"],
        "catalogo_items"
    );

    let (status, listado) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/packs?vertical_id={vertical}"),
        Value::Null,
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listado["data"]
        .as_array()
        .expect("packs")
        .iter()
        .any(|item| item["codigo"] == pack));

    let (status, obtenido) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/packs/{pack}/1"),
        Value::Null,
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(obtenido["data"]["vertical_codigo"], vertical);

    let comprador = autorizacion(&Uuid::new_v4().to_string(), "comprador");
    let (status, _) = enviar(
        &ctx.app,
        "GET",
        "/api/v2/packs",
        Value::Null,
        &headers(&comprador),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn debe_listar_y_actualizar_capacidades_por_empresa() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let admin_usuario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Admin de prueba".into(),
            correo: format!("admin-{}@tienda.test", Uuid::new_v4()),
            contrasena: "secreto123".into(),
            rol: "super_admin".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("admin");
    let admin_id = admin_usuario.id;
    let empresa = Uuid::new_v4().to_string();
    let admin = autorizacion(&admin_id, "super_admin");
    let mut tx = ctx.db.transaccion().await.expect("tx");
    sqlx::query(
        "INSERT INTO identity.empresas
           (id, nombre, slug, vertical_id, pack_codigo, pack_version, creado_por)
         VALUES ($1::uuid, 'Empresa de prueba', $2, 'retail', 'pack_retail', 1, $3::uuid)",
    )
    .bind(&empresa)
    .bind(format!("empresa-{}", &empresa[..8]))
    .bind(&admin_id)
    .execute(&mut *tx)
    .await
    .expect("empresa");
    sqlx::query(
        "INSERT INTO identity.empresa_capacidades
           (empresa_id, capacidad_codigo, activo, config, actualizado_en)
         VALUES ($1::uuid, 'catalogo_items', true, '{}'::jsonb, NOW())",
    )
    .bind(&empresa)
    .execute(&mut *tx)
    .await
    .expect("capacidad");
    tx.commit().await.expect("commit");

    let (status, capacidades) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa}/capacidades"),
        Value::Null,
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(capacidades["data"]
        .as_array()
        .expect("capacidades")
        .iter()
        .any(|item| item["capacidad_codigo"] == "catalogo_items"));

    let (status, actualizada) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa}/capacidades/catalogo_items"),
        json!({ "activo": false }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(actualizada["data"]["activo"], false);

    let (status, _) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa}/capacidades/no_existe"),
        json!({ "activo": true }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let comprador = autorizacion(&Uuid::new_v4().to_string(), "comprador");
    let (status, _) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa}/capacidades"),
        Value::Null,
        &headers(&comprador),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn debe_listar_usuarios_y_limitar_el_avatar() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let correo_a = correo();
    let correo_b = correo();
    let (_, a) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/registro",
        json!({
            "nombre": "Ana",
            "correo": correo_a,
            "contrasena": "secreto123",
            "rol": "comprador"
        }),
        &[],
    )
    .await;
    let (_, b) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/registro",
        json!({
            "nombre": "Beto",
            "correo": correo_b,
            "contrasena": "secreto123",
            "rol": "comprador"
        }),
        &[],
    )
    .await;
    let id_a = a["data"]["usuario"]["id"].as_str().expect("a").to_string();
    let id_b = b["data"]["usuario"]["id"].as_str().expect("b").to_string();
    let admin = autorizacion(&Uuid::new_v4().to_string(), "admin");

    let (status, listado) = enviar(
        &ctx.app,
        "GET",
        "/api/v2/usuarios",
        Value::Null,
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listado["data"]
        .as_array()
        .expect("usuarios")
        .iter()
        .any(|item| item["id"] == id_a));

    let auth_a = autorizacion(&id_a, "comprador");
    let (status, propio) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/usuarios/{id_a}"),
        Value::Null,
        &headers(&auth_a),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(propio["data"]["id"], id_a);

    let auth_b = autorizacion(&id_b, "comprador");
    let (status, _) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/usuarios/{id_a}"),
        Value::Null,
        &headers(&auth_b),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = enviar(
        &ctx.app,
        "GET",
        "/api/v2/usuarios",
        Value::Null,
        &headers(&auth_a),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/usuarios/{id_a}/avatar"),
        json!({ "avatar_objeto_id": null }),
        &headers(&auth_b),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, error) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/usuarios/{id_a}/avatar"),
        json!({ "avatar_objeto_id": "no-uuid" }),
        &headers(&auth_a),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "VALIDACION");

    let (status, error) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/usuarios/{id_a}/avatar"),
        json!({ "avatar_objeto_id": Uuid::new_v4().to_string() }),
        &headers(&auth_a),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "TENANT_REQUERIDO");

    sqlx::query("UPDATE identity.usuarios SET tenant_id = $1::uuid WHERE id = $2::uuid")
        .bind(Uuid::new_v4().to_string())
        .bind(&id_a)
        .execute(ctx.db.pool())
        .await
        .expect("tenant");

    let (status, error) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/usuarios/{id_a}/avatar"),
        json!({ "avatar_objeto_id": Uuid::new_v4().to_string() }),
        &headers(&auth_a),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "AVATAR_NO_VERIFICABLE");
}

#[tokio::test]
async fn debe_crear_listar_y_canjear_api_keys() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let empresa = Uuid::new_v4().to_string();
    let correo = correo();
    let empresario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Empresario".into(),
            correo,
            contrasena: "secreto123".into(),
            rol: "empresario".into(),
            tenant_id: Some(empresa.clone()),
            personal_id: None,
        })
        .await
        .expect("empresario");
    sqlx::query(
        "INSERT INTO identity.empresas
           (id, nombre, slug, vertical_id, pack_codigo, pack_version, creado_por)
         VALUES ($1::uuid, 'Empresa API', $2, 'retail', 'pack_retail', 1, $3::uuid)",
    )
    .bind(&empresa)
    .bind(format!("empresa-api-{}", &empresa[..8]))
    .bind(&empresario.id)
    .execute(ctx.db.pool())
    .await
    .expect("empresa");

    let mut headers_actor = autorizacion(&empresario.id, "empresario");
    headers_actor.push(("x-tenant", empresa.clone()));
    let actor_headers = headers(&headers_actor);

    let (status, creada) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/api-keys",
        json!({ "nombre": "Integración de prueba", "rol": "operativo" }),
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let key = creada["data"]["key"].as_str().expect("key").to_string();
    let id = creada["data"]["id"].as_str().expect("id").to_string();
    assert!(key.starts_with("cek_"));
    assert_eq!(creada["data"]["empresa_id"], empresa);
    assert_eq!(creada["data"]["estado"], "activa");

    let (status, listado) = enviar(
        &ctx.app,
        "GET",
        "/api/v2/api-keys",
        Value::Null,
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listado["data"]
        .as_array()
        .expect("keys")
        .iter()
        .any(|item| item["id"] == id));

    let (status, token) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/token",
        Value::Null,
        &[("x-api-key", key.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(token["data"]["token_type"], "Bearer");
    assert_eq!(token["data"]["expires_in"], 900);
    let payload = identity_service::auth::verificar_token(
        token["data"]["access_token"].as_str().expect("jwt"),
        "secreto-de-prueba",
    )
    .expect("jwt");
    assert_eq!(payload.rol, "operativo");
    assert_eq!(payload.tenant_id.as_deref(), Some(empresa.as_str()));

    let (status, revocada) = enviar(
        &ctx.app,
        "DELETE",
        &format!("/api/v2/api-keys/{id}"),
        Value::Null,
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(revocada["data"]["estado"], "revocada");

    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/auth/token",
        Value::Null,
        &[("x-api-key", key.as_str())],
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, error) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/api-keys",
        json!({ "nombre": "Integración", "rol": "admin" }),
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "VALIDACION");

    let admin = autorizacion(&Uuid::new_v4().to_string(), "admin");
    let (status, error) = enviar(
        &ctx.app,
        "GET",
        "/api/v2/api-keys",
        Value::Null,
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "EMPRESA_REQUERIDA");
}

#[tokio::test]
async fn debe_leer_y_actualizar_la_configuracion_del_sitio() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let (status, inicial) = enviar(
        &ctx.app,
        "GET",
        "/api/v2/configuracion/sitio",
        Value::Null,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(inicial["data"]["background_overlay"], 35);
    assert_eq!(inicial["data"]["plantilla"], "clasica");
    assert_eq!(inicial["data"]["origen_personalizacion"]["tema"], "defecto");

    let admin = autorizacion(&Uuid::new_v4().to_string(), "admin");
    let (status, actualizada) = enviar(
        &ctx.app,
        "PATCH",
        "/api/v2/configuracion/sitio",
        json!({
            "plantilla": "minimal",
            "tema": { "color_primario": "#ABCDEF", "radio_bordes": 8 },
            "background_alt": "Portada",
            "background_overlay": 50
        }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(actualizada["data"]["plantilla"], "minimal");
    assert_eq!(actualizada["data"]["tema"]["color_primario"], "#ABCDEF");
    assert_eq!(actualizada["data"]["tema"]["radio_bordes"], 8);
    assert_eq!(actualizada["data"]["background_alt"], "Portada");
    assert_eq!(actualizada["data"]["background_overlay"], 50);
    assert_eq!(
        actualizada["data"]["origen_personalizacion"]["tema"],
        "plataforma"
    );

    let (status, error) = enviar(
        &ctx.app,
        "PATCH",
        "/api/v2/configuracion/sitio",
        json!({ "background_activo": true }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "BACKGROUND_INVALIDO");

    let (status, error) = enviar(
        &ctx.app,
        "PATCH",
        "/api/v2/configuracion/sitio",
        json!({ "tema": { "color_primario": "rojo" } }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "PERSONALIZACION_INVALIDA");

    let (status, error) = enviar(
        &ctx.app,
        "PATCH",
        "/api/v2/configuracion/sitio",
        json!({ "background_objeto_id": Uuid::new_v4().to_string() }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "PLATFORM_TENANT_ID_NO_CONFIGURADO");

    let comprador = autorizacion(&Uuid::new_v4().to_string(), "comprador");
    let (status, _) = enviar(
        &ctx.app,
        "PATCH",
        "/api/v2/configuracion/sitio",
        json!({ "plantilla": "minimal" }),
        &headers(&comprador),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn debe_crear_gestionar_y_resolver_empresas() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let correo_empresario = correo();
    let empresario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Empresario".into(),
            correo: correo_empresario,
            contrasena: "secreto123".into(),
            rol: "empresario".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("empresario");
    let actor = autorizacion(&empresario.id, "empresario");
    let slug = format!("empresa-{}", &Uuid::new_v4().simple().to_string()[..8]);

    let (status, creada) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/empresas",
        json!({
            "nombre": "Empresa de prueba",
            "slug": slug,
            "vertical_id": "retail",
            "pack_codigo": "pack_retail",
            "ruc": "RUC-001",
            "moneda": "nio"
        }),
        &headers(&actor),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let empresa_id = creada["data"]["id"].as_str().expect("id").to_string();
    assert_eq!(creada["data"]["slug"], slug);
    assert_eq!(creada["data"]["vertical_id"], "retail");
    assert_eq!(creada["data"]["pack_codigo"], "pack_retail");
    assert_eq!(creada["data"]["moneda"], "NIO");
    assert_eq!(creada["data"]["plantilla"], "clasica");
    assert_eq!(creada["data"]["background_overlay"], 35);
    assert_eq!(
        creada["data"]["origen_personalizacion"]["plantilla"],
        "defecto"
    );

    let actualizado = ctx
        .usuarios
        .encontrar_por_id(&empresario.id)
        .await
        .expect("usuario")
        .expect("existe");
    assert_eq!(actualizado.tenant_id.as_deref(), Some(empresa_id.as_str()));
    assert_eq!(actualizado.rol, "empresario");

    let (status, por_id) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}"),
        Value::Null,
        &headers(&actor),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(por_id["data"]["id"], empresa_id);

    let (status, por_slug) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{slug}"),
        Value::Null,
        &headers(&actor),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(por_slug["data"]["slug"], slug);

    let (status, perfil) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa_id}/perfil"),
        json!({
            "razon_social": "Empresa de prueba S.A.",
            "plantilla": "boutique",
            "tema": { "color_primario": "#112233", "radio_bordes": 4 }
        }),
        &headers(&actor),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(perfil["data"]["razon_social"], "Empresa de prueba S.A.");
    assert_eq!(perfil["data"]["plantilla"], "boutique");
    assert_eq!(perfil["data"]["tema"]["color_primario"], "#112233");

    let nuevo_slug = format!("empresa-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let (status, cambiada) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa_id}/slug"),
        json!({ "slug": nuevo_slug }),
        &headers(&actor),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cambiada["data"]["slug"], nuevo_slug);

    let (status, redirigida) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{slug}"),
        Value::Null,
        &headers(&actor),
    )
    .await;
    assert_eq!(status, StatusCode::MOVED_PERMANENTLY);
    assert_eq!(redirigida["slug"], nuevo_slug);

    let (status, vendedor) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/vendedores"),
        json!({
            "nombre": "Vendedor de prueba",
            "correo": correo(),
            "contrasena": "secreto123",
            "participacion_porcentaje": 35
        }),
        &headers(&actor),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(vendedor["data"]["rol"], "vendedor");
    assert_eq!(vendedor["data"]["participacion_porcentaje"], 35);

    sqlx::query(
        "INSERT INTO identity.packs (codigo, version, vertical_codigo, nombre)
         VALUES ('pack_retail', 2, 'retail', 'Retail v2') ON CONFLICT DO NOTHING",
    )
    .execute(ctx.db.pool())
    .await
    .expect("pack v2");
    let admin = autorizacion(&Uuid::new_v4().to_string(), "admin");
    let (status, pack) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa_id}/pack"),
        json!({ "pack_codigo": "pack_retail", "pack_version": 2 }),
        &headers(&admin),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pack["data"]["pack_version"], 2);
}

#[tokio::test]
async fn debe_negar_empresas_a_quien_no_es_miembro() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let empresario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Dueño".into(),
            correo: correo(),
            contrasena: "secreto123".into(),
            rol: "empresario".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("empresario");
    let actor = autorizacion(&empresario.id, "empresario");
    let slug = format!("empresa-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let (_, creada) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/empresas",
        json!({
            "nombre": "Empresa privada",
            "slug": slug,
            "vertical_id": "retail",
            "pack_codigo": "pack_retail"
        }),
        &headers(&actor),
    )
    .await;
    let empresa_id = creada["data"]["id"].as_str().expect("id").to_string();

    let otro = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Otro".into(),
            correo: correo(),
            contrasena: "secreto123".into(),
            rol: "empresario".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("otro");
    let otro_actor = autorizacion(&otro.id, "empresario");
    let (status, _) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}"),
        Value::Null,
        &headers(&otro_actor),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa_id}/perfil"),
        json!({ "razon_social": "No permitido" }),
        &headers(&otro_actor),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let comprador = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Comprador".into(),
            correo: correo(),
            contrasena: "secreto123".into(),
            rol: "comprador".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("comprador");
    let comprador_actor = autorizacion(&comprador.id, "comprador");
    let (status, _) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/empresas",
        json!({
            "nombre": "Otra empresa",
            "vertical_id": "retail",
            "pack_codigo": "pack_retail"
        }),
        &headers(&comprador_actor),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn debe_gestionar_personal_e_incorporaciones() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let correo_empresario = correo();
    let empresario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Empresario".into(),
            correo: correo_empresario,
            contrasena: "secreto123".into(),
            rol: "empresario".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("empresario");
    let actor = autorizacion(&empresario.id, "empresario");
    let actor_headers = headers(&actor);
    let slug = format!("empresa-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let (_, creada) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/empresas",
        json!({
            "nombre": "Empresa personal",
            "slug": slug,
            "vertical_id": "retail",
            "pack_codigo": "pack_retail"
        }),
        &actor_headers,
    )
    .await;
    let empresa_id = creada["data"]["id"].as_str().expect("empresa").to_string();
    let mut actor_pares = actor.clone();
    actor_pares.push(("x-tenant", empresa_id.clone()));
    let actor_headers = headers(&actor_pares);

    let (status, rol) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/roles"),
        json!({
            "codigo": "estilista",
            "nombre": "Estilista",
            "descripcion": "Atiende citas",
            "permisos": {"acciones": ["agenda.leer", "agenda.escribir"]}
        }),
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let rol_id = rol["data"]["id"].as_str().expect("rol").to_string();
    assert_eq!(rol["data"]["codigo"], "estilista");

    let (status, _) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa_id}/roles/{rol_id}"),
        json!({ "activo": true }),
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, puesto) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/puestos"),
        json!({ "codigo": "sillon_1", "nombre": "Sillón 1", "tipo": "sillon" }),
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let puesto_id = puesto["data"]["id"].as_str().expect("puesto").to_string();

    let (status, vendedor) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/vendedores"),
        json!({
            "nombre": "Vendedor",
            "correo": correo(),
            "contrasena": "secreto123",
            "participacion_porcentaje": 20
        }),
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let vendedor_id = vendedor["data"]["id"]
        .as_str()
        .expect("vendedor")
        .to_string();

    let (status, asignacion) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/asignaciones-personal"),
        json!({
            "usuario_id": vendedor_id,
            "rol_empresa_id": rol_id,
            "puesto_id": puesto_id
        }),
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(asignacion["data"]["estado"], "activa");

    let (status, asignaciones) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}/asignaciones-personal"),
        Value::Null,
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(asignaciones["data"]
        .as_array()
        .expect("asignaciones")
        .iter()
        .any(|item| item["usuario_id"] == vendedor_id));

    let correo_invitado = correo();
    let (status, invitacion) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/invitaciones/personal"),
        json!({
            "correo": correo_invitado,
            "nombre": "Nuevo",
            "rol_empresa_id": rol_id,
            "puesto_id": puesto_id
        }),
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(invitacion["data"]["url_invitacion"]
        .as_str()
        .expect("url")
        .contains("/incorporacion/"));

    let mensaje = ctx
        .email
        .enviados
        .lock()
        .expect("email")
        .last()
        .cloned()
        .expect("correo invitación");
    let token = mensaje
        .text
        .split("/incorporacion/")
        .nth(1)
        .and_then(|resto| resto.split_whitespace().next())
        .expect("token")
        .to_string();

    let (status, consulta) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/incorporaciones/{token}"),
        Value::Null,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(consulta["data"]["valida"], true);
    assert_eq!(consulta["data"]["correo"], correo_invitado);

    let (status, sesion) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/incorporaciones/{token}/aceptar"),
        json!({ "nombre": "Nuevo Operativo", "contrasena": "secreto123" }),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(sesion["data"]["usuario"]["rol"], "operativo");
    assert_eq!(sesion["data"]["usuario"]["rol_empresa_id"], rol_id);
    assert_eq!(sesion["data"]["usuario"]["puesto_id"], puesto_id);

    let (status, segunda) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/invitaciones/personal"),
        json!({
            "correo": correo(),
            "rol_empresa_id": rol_id
        }),
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let segunda_id = segunda["data"]["id"]
        .as_str()
        .expect("invitación")
        .to_string();
    let (status, revocada) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/invitaciones/personal/{segunda_id}/revocar"),
        Value::Null,
        &actor_headers,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(revocada["data"]["ok"], true);
}

#[tokio::test]
async fn debe_gestionar_partes_contactos_direcciones_y_relaciones() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let empresario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Empresario".into(),
            correo: correo(),
            contrasena: "secreto123".into(),
            rol: "empresario".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("empresario");
    let actor_base = autorizacion(&empresario.id, "empresario");
    let slug = format!("empresa-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let (_, creada) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/empresas",
        json!({
            "nombre": "Empresa partes",
            "slug": slug,
            "vertical_id": "retail",
            "pack_codigo": "pack_retail",
            "pack_version": 1
        }),
        &headers(&actor_base),
    )
    .await;
    let empresa_id = creada["data"]["id"].as_str().expect("empresa").to_string();
    let mut actor_pares = actor_base.clone();
    actor_pares.push(("x-tenant", empresa_id.clone()));
    let actor = headers(&actor_pares);

    let (status, parte) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/partes"),
        json!({
            "tipo": "persona",
            "nombre": "Ana Pérez",
            "nombre_comercial": "Ana",
            "identificador_tipo": "cedula",
            "identificador_valor": "001-010190-0001A"
        }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let parte_id = parte["data"]["id"].as_str().expect("parte").to_string();
    assert_eq!(parte["data"]["estado"], "activo");

    let (status, listado) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}/partes?q=Ana"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listado["data"].as_array().expect("partes").len(), 1);

    let (status, actualizada) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}"),
        json!({ "nombre": "Ana María", "estado": "inactivo" }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(actualizada["data"]["nombre"], "Ana María");
    assert_eq!(actualizada["data"]["estado"], "inactivo");

    let (status, contacto) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/contactos"),
        json!({ "tipo": "email", "valor": "ana@test.dev", "es_principal": true }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let contacto_id = contacto["data"]["id"]
        .as_str()
        .expect("contacto")
        .to_string();
    let (status, contactos) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/contactos"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(contactos["data"].as_array().expect("contactos").len(), 1);
    let (status, _) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/contactos/{contacto_id}"),
        json!({ "etiqueta": "trabajo" }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, direccion) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/direcciones"),
        json!({ "tipo": "entrega", "linea_1": "Calle 1", "latitud": 12.1 }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let direccion_id = direccion["data"]["id"]
        .as_str()
        .expect("direccion")
        .to_string();
    let (status, _) = enviar(
        &ctx.app,
        "DELETE",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/direcciones/{direccion_id}"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (_, otra) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/partes"),
        json!({ "tipo": "organizacion", "nombre": "Proveedor S.A." }),
        &actor,
    )
    .await;
    let otra_id = otra["data"]["id"].as_str().expect("otra").to_string();
    let (status, relacion) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/relaciones"),
        json!({ "parte_destino_id": otra_id, "tipo": "proveedor" }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let relacion_id = relacion["data"]["id"]
        .as_str()
        .expect("relacion")
        .to_string();
    let (status, relaciones) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/relaciones"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(relaciones["data"][0]["parte_relacionada"]["id"], otra_id);
    let (status, _) = enviar(
        &ctx.app,
        "DELETE",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/relaciones/{relacion_id}"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = enviar(
        &ctx.app,
        "DELETE",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn debe_gestionar_registros_y_exigir_capacidad() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let empresario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Empresario".into(),
            correo: correo(),
            contrasena: "secreto123".into(),
            rol: "empresario".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("empresario");
    let actor_base = autorizacion(&empresario.id, "empresario");
    let slug = format!("empresa-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let (_, creada) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/empresas",
        json!({
            "nombre": "Empresa registros",
            "slug": slug,
            "vertical_id": "retail",
            "pack_codigo": "pack_retail",
            "pack_version": 1
        }),
        &headers(&actor_base),
    )
    .await;
    let empresa_id = creada["data"]["id"].as_str().expect("empresa").to_string();
    let mut actor_pares = actor_base.clone();
    actor_pares.push(("x-tenant", empresa_id.clone()));
    let actor = headers(&actor_pares);
    let (_, parte) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/partes"),
        json!({ "tipo": "persona", "nombre": "Paciente" }),
        &actor,
    )
    .await;
    let parte_id = parte["data"]["id"].as_str().expect("parte").to_string();

    let (status, registro) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/registros"),
        json!({
            "tipo": "nota",
            "titulo": "Primera nota",
            "datos": {"texto": "ok"},
            "adjuntos": ["11111111-1111-4111-8111-111111111111"]
        }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let registro_id = registro["data"]["id"]
        .as_str()
        .expect("registro")
        .to_string();
    assert_eq!(registro["data"]["tipo"], "nota");

    let (status, listado) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/registros"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listado["data"].as_array().expect("registros").len(), 1);

    let (status, actualizado) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/registros/{registro_id}"),
        json!({ "titulo": "Nota corregida", "datos": {"texto": "corregido"} }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(actualizado["data"]["titulo"], "Nota corregida");

    let (status, error) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/registros"),
        json!({ "tipo": "historia_clinica", "datos": {} }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["codigo"], "CAPACIDAD_NO_ACTIVA");

    let (status, _) = enviar(
        &ctx.app,
        "DELETE",
        &format!("/api/v2/empresas/{empresa_id}/partes/{parte_id}/registros/{registro_id}"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn debe_gestionar_extensiones_y_validar_json_schema() {
    let Some(ctx) = preparar().await else {
        return;
    };
    let empresario = ctx
        .usuarios
        .crear(CrearUsuario {
            nombre: "Empresario".into(),
            correo: correo(),
            contrasena: "secreto123".into(),
            rol: "empresario".into(),
            tenant_id: None,
            personal_id: None,
        })
        .await
        .expect("empresario");
    let actor_base = autorizacion(&empresario.id, "empresario");
    let slug = format!("empresa-{}", &Uuid::new_v4().simple().to_string()[..8]);
    let (_, creada) = enviar(
        &ctx.app,
        "POST",
        "/api/v2/empresas",
        json!({
            "nombre": "Empresa extensiones",
            "slug": slug,
            "vertical_id": "retail",
            "pack_codigo": "pack_retail",
            "pack_version": 1
        }),
        &headers(&actor_base),
    )
    .await;
    let empresa_id = creada["data"]["id"].as_str().expect("empresa").to_string();
    let mut actor_pares = actor_base.clone();
    actor_pares.push(("x-tenant", empresa_id.clone()));
    let actor = headers(&actor_pares);

    let (status, extension) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/extensiones"),
        json!({
            "codigo": "ficha_cliente",
            "nombre": "Ficha de cliente",
            "recurso": "cliente",
            "esquema": {
                "type": "object",
                "properties": {"edad": {"type": "integer", "minimum": 0}},
                "required": ["edad"],
                "additionalProperties": false
            }
        }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(extension["data"]["codigo"], "ficha_cliente");
    assert_eq!(extension["data"]["version"], 1);
    assert_eq!(extension["data"]["activo"], true);

    let (status, listado) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}/extensiones"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listado["data"].as_array().expect("extensiones").len(), 1);

    let (status, valido) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/extensiones/ficha_cliente/validar"),
        json!({ "datos": { "edad": 20 } }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valido["data"]["valido"], true);

    let (status, invalido) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/extensiones/ficha_cliente/validar"),
        json!({ "datos": { "edad": -1 } }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(invalido["data"]["valido"], false);
    assert!(!invalido["data"]["errores"]
        .as_array()
        .expect("errores")
        .is_empty());

    let recurso_id = Uuid::new_v4().to_string();
    let (status, valores) = enviar(
        &ctx.app,
        "PUT",
        &format!("/api/v2/empresas/{empresa_id}/extensiones/ficha_cliente/valores/{recurso_id}"),
        json!({ "valores": { "edad": 20 } }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valores["data"]["valores"]["edad"], 20);

    let (status, guardados) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}/extensiones/ficha_cliente/valores/{recurso_id}"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(guardados["data"]["version_schema"], 1);

    let (status, error) = enviar(
        &ctx.app,
        "PUT",
        &format!("/api/v2/empresas/{empresa_id}/extensiones/ficha_cliente/valores/{recurso_id}"),
        json!({ "valores": { "edad": -1 } }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "VALORES_EXTENSION_INVALIDOS");

    let (status, error) = enviar(
        &ctx.app,
        "POST",
        &format!("/api/v2/empresas/{empresa_id}/extensiones"),
        json!({
            "codigo": "con_ref",
            "nombre": "Con ref",
            "recurso": "cliente",
            "esquema": {"$ref": "https://example.com/schema.json"}
        }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["codigo"], "ESQUEMA_EXTENSION_INVALIDO");

    let (status, _) = enviar(
        &ctx.app,
        "PATCH",
        &format!("/api/v2/empresas/{empresa_id}/extensiones/ficha_cliente"),
        json!({ "activo": false }),
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = enviar(
        &ctx.app,
        "GET",
        &format!("/api/v2/empresas/{empresa_id}/extensiones/ficha_cliente/valores/{recurso_id}"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = enviar(
        &ctx.app,
        "DELETE",
        &format!("/api/v2/empresas/{empresa_id}/extensiones/ficha_cliente/valores/{recurso_id}"),
        Value::Null,
        &actor,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
