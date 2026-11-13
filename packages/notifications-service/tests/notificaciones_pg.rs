//! Pruebas de integración contra PostgreSQL (solo si `TEST_DATABASE_URL` está
//! definida y el esquema `notifications` existe o puede crearse). El runner de
//! CI las ejecuta con la BD efímera.

use async_trait::async_trait;
use notifications_service::consumidor::ConsumidorNotificaciones;
use notifications_service::db::Db;
use notifications_service::errors::ErrorNotificacion;
use notifications_service::notificaciones::NotificacionesPort;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
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
