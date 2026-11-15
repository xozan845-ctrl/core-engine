//! Pruebas HTTP de paridad contra PostgreSQL (solo con `TEST_DATABASE_URL`).
//! Ejercitan el router completo: guards, validación, envelope y handlers.

use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use field_service::auth::{CapacidadActiva, Capacidades};
use field_service::clients::Partes;
use field_service::config::Config;
use field_service::db::Db;
use field_service::field::FieldService;
use field_service::http::build_router;
use field_service::metrics::Metrics;
use field_service::state::AppState;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

static MIGRACION: OnceLock<Mutex<()>> = OnceLock::new();

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

struct PartesPermitidas;

#[async_trait]
impl Partes for PartesPermitidas {
    async fn pertenece(&self, _: &str, _: &str) -> Result<bool, ()> {
        Ok(true)
    }
}

struct CapacidadesFijas(Vec<&'static str>, String);

#[async_trait]
impl Capacidades for CapacidadesFijas {
    async fn activas(
        &self,
        empresa_id: &str,
        _requeridas: &[&str],
    ) -> Result<Vec<CapacidadActiva>, ()> {
        if empresa_id != self.1 {
            return Ok(Vec::new());
        }
        Ok(self
            .0
            .iter()
            .map(|codigo| CapacidadActiva {
                empresa_id: empresa_id.to_string(),
                capacidad_codigo: (*codigo).to_string(),
                activo: true,
            })
            .collect())
    }
}

async fn preparar(url: &str) -> (Arc<Db>, PgPool) {
    let pool = PgPool::connect(url).await.expect("postgres");
    let _guard = MIGRACION.get_or_init(|| Mutex::new(())).lock().await;
    sqlx::raw_sql(include_str!("../../../infra/db/init/02_field.sql"))
        .execute(&pool)
        .await
        .expect("migracion field");
    for tabla in ["clientes", "paradas", "pedidos", "visitas"] {
        sqlx::query(&format!(
            "ALTER TABLE field.{tabla} ADD COLUMN IF NOT EXISTS parte_id uuid"
        ))
        .execute(&pool)
        .await
        .expect("columna parte_id");
    }
    let db = Arc::new(Db::conectar(url).await.expect("pool del servicio"));
    (db, pool)
}

async fn app(db: Arc<Db>, capacidades: Arc<dyn Capacidades>) -> axum::Router {
    let config = Config::from_pairs([("SERVICIO", "field-service")]).expect("config");
    let metrics = Arc::new(Metrics::nueva("field-service"));
    let field = Arc::new(FieldService::nuevo(db, Arc::new(PartesPermitidas)));
    build_router(AppState::con_dependencias(
        config,
        metrics,
        field,
        capacidades,
    ))
}

fn headers(rol: &str, tenant: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "x-user-id",
            "11111111-1111-4111-8111-111111111111".to_string(),
        ),
        ("x-user-email", "admin@test.dev".to_string()),
        ("x-user-rol", rol.to_string()),
        ("x-tenant", tenant.to_string()),
    ]
}

async fn pedir(
    app: &axum::Router,
    metodo: Method,
    ruta: &str,
    cabeceras: Vec<(&'static str, String)>,
    cuerpo: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(metodo).uri(ruta);
    for (clave, valor) in cabeceras {
        builder = builder.header(clave, valor);
    }
    let body = match cuerpo {
        Some(valor) => Body::from(valor.to_string()),
        None => Body::empty(),
    };
    let request = builder.body(body).expect("request");
    let response = app.clone().oneshot(request).await.expect("response");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("cuerpo")
        .to_bytes();
    let valor = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, valor)
}

#[tokio::test]
async fn debe_ejecutar_el_crud_http_con_envelope_y_errores() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let tenant = Uuid::new_v4().to_string();
    let app = app(
        Arc::clone(&db),
        Arc::new(CapacidadesFijas(Vec::new(), tenant.clone())),
    )
    .await;

    let (status, valor) = pedir(
        &app,
        Method::POST,
        "/api/v2/field/personal",
        headers("admin", &tenant),
        Some(json!({"nombre": "Ana", "email": "ana@test.dev"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(valor["data"]["nombre"], "Ana");
    assert_eq!(valor["data"]["cargo"], "conductor");
    assert_eq!(valor["meta"]["statusCode"], 201);
    assert_eq!(valor["meta"]["method"], "POST");
    assert!(valor["meta"]["timestamp"]
        .as_str()
        .expect("timestamp")
        .ends_with('Z'));
    let id = valor["data"]["id"].as_str().expect("id").to_string();

    let (status, valor) = pedir(
        &app,
        Method::GET,
        "/api/v2/field/personal",
        headers("admin", &tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["data"].as_array().expect("lista").len(), 1);

    let (status, valor) = pedir(
        &app,
        Method::PATCH,
        &format!("/api/v2/field/personal/{id}"),
        headers("admin", &tenant),
        Some(json!({"estado": "en_ruta"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["data"]["estado"], "en_ruta");

    let (status, valor) = pedir(
        &app,
        Method::GET,
        &format!("/api/v2/field/personal/{id}/ubicacion"),
        headers("admin", &tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["data"]["personalId"], id);
    assert!(valor["data"]["latitud"].is_null());

    let (status, _) = pedir(
        &app,
        Method::DELETE,
        &format!("/api/v2/field/personal/{id}"),
        headers("admin", &tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, valor) = pedir(
        &app,
        Method::GET,
        &format!("/api/v2/field/personal/{id}"),
        headers("admin", &tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(valor["codigo"], "NO_ENCONTRADO");
    assert!(valor.get("meta").is_none());

    let (status, valor) = pedir(
        &app,
        Method::POST,
        "/api/v2/field/personal",
        headers("admin", &tenant),
        Some(json!({"email": "sin-nombre"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(valor["codigo"], "VALIDACION");
    assert_eq!(valor["detalles"][0]["campo"], "nombre");

    let (status, valor) = pedir(
        &app,
        Method::POST,
        "/api/v2/field/personal",
        headers("comprador", &tenant),
        Some(json!({"nombre": "Ana"})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(valor["codigo"], "ACCESO_DENEGADO");

    db.cerrar().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_ejecutar_rutas_pedidos_tracking_y_sync_por_http() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let tenant = Uuid::new_v4().to_string();
    let app = app(
        Arc::clone(&db),
        Arc::new(CapacidadesFijas(
            vec!["operacion_campo", "logistica_rutas"],
            tenant.clone(),
        )),
    )
    .await;

    let (status, ruta) = pedir(
        &app,
        Method::POST,
        "/api/v2/field/rutas",
        headers("supervisor", &tenant),
        Some(json!({"nombre": "Norte", "personalIds": ["p-1"]})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let ruta_id = ruta["data"]["id"].as_str().expect("id").to_string();

    let (status, parada) = pedir(
        &app,
        Method::POST,
        &format!("/api/v2/field/rutas/{ruta_id}/paradas"),
        headers("supervisor", &tenant),
        Some(json!({"nombre": "Centro", "orden": 1})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(parada["data"]["ruta_id"], ruta_id);

    let (status, valor) = pedir(
        &app,
        Method::GET,
        &format!("/api/v2/field/rutas/{ruta_id}"),
        headers("logistica", &tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        valor["data"]["paradas"].as_array().expect("paradas").len(),
        1
    );

    let (status, pedido) = pedir(
        &app,
        Method::POST,
        "/api/v2/field/pedidos",
        headers("operativo", &tenant),
        Some(json!({"cliente": "Ana", "rutaId": ruta_id})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let pedido_id = pedido["data"]["id"].as_str().expect("id").to_string();

    let (status, valor) = pedir(
        &app,
        Method::PATCH,
        &format!("/api/v2/field/pedidos/{pedido_id}/estado"),
        headers("operativo", &tenant),
        Some(json!({"estado": "en_camino"})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(valor["data"]["estado"], "en_camino");

    let personal_tracking = Uuid::new_v4().to_string();
    let (status, valor) = pedir(
        &app,
        Method::POST,
        "/api/v2/field/tracking",
        headers("logistica", &tenant),
        Some(json!({"registros": [{"personalId": personal_tracking, "latitud": 12.1, "longitud": -86.2}]})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(valor["data"].as_array().expect("tracking").len(), 1);

    let cliente_id = Uuid::new_v4().to_string();
    let (status, valor) = pedir(
        &app,
        Method::POST,
        "/api/v2/field/sync",
        headers("logistica", &tenant),
        Some(json!({
            "operaciones": [
                {"tipo": "cliente", "payload": {"id": cliente_id, "nombreCompleto": "Ana"}},
                {"tipo": "desconocido", "payload": {}},
            ]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(valor["data"][0]["ok"], true);
    assert_eq!(valor["data"][1]["ok"], false);

    db.cerrar().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_exigir_la_capacidad_de_rutas_y_la_de_campo() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let tenant = Uuid::new_v4().to_string();

    let sin_rutas = app(
        Arc::clone(&db),
        Arc::new(CapacidadesFijas(vec!["operacion_campo"], tenant.clone())),
    )
    .await;
    let (status, valor) = pedir(
        &sin_rutas,
        Method::GET,
        "/api/v2/field/rutas",
        headers("logistica", &tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(valor["codigo"], "CAPACIDAD_NO_ACTIVA");
    assert_eq!(valor["detalles"]["capacidades"], json!(["logistica_rutas"]));

    let con_rutas = app(
        Arc::clone(&db),
        Arc::new(CapacidadesFijas(
            vec!["operacion_campo", "logistica_rutas"],
            tenant.clone(),
        )),
    )
    .await;
    let (status, _) = pedir(
        &con_rutas,
        Method::GET,
        "/api/v2/field/rutas",
        headers("logistica", &tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let sin_campo = app(
        Arc::clone(&db),
        Arc::new(CapacidadesFijas(vec!["logistica_rutas"], tenant.clone())),
    )
    .await;
    let (status, valor) = pedir(
        &sin_campo,
        Method::GET,
        "/api/v2/field/rutas",
        headers("logistica", &tenant),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(valor["detalles"]["capacidades"], json!(["operacion_campo"]));

    db.cerrar().await;
    pool.close().await;
}
