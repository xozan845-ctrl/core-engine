//! Pruebas de integración contra PostgreSQL (solo si `TEST_DATABASE_URL` está
//! definida). Provisionan su propio esquema `logistics` para correr en CI.

use async_trait::async_trait;
use logistics_service::auth::{CapacidadActiva, Capacidades};
use logistics_service::clients::{OrdenExterna, OrdenItemExterna, Orders};
use logistics_service::consumidor::LogisticsConsumer;
use logistics_service::db::Db;
use logistics_service::envios::Envios;
use logistics_service::errors::ApiError;
use logistics_service::outbox::Outbox;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

static DB_LOCK: Mutex<()> = Mutex::const_new(());

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

struct CapacidadesFijas(Vec<CapacidadActiva>);

#[async_trait]
impl Capacidades for CapacidadesFijas {
    async fn activas(
        &self,
        _empresa_id: &str,
        _requeridas: &[&str],
    ) -> Result<Vec<CapacidadActiva>, ()> {
        Ok(self.0.clone())
    }
}

#[derive(Default)]
struct OrdersFake {
    orden: Mutex<Option<OrdenExterna>>,
    avances: Mutex<Vec<Avance>>,
}

type Avance = (String, String, Option<String>, Option<String>);

#[async_trait]
impl Orders for OrdersFake {
    async fn consultar(&self, _order_id: &str) -> Result<Option<OrdenExterna>, ()> {
        Ok(self.orden.lock().await.clone())
    }

    async fn avanzar(
        &self,
        order_id: &str,
        estado: &str,
        motivo: Option<&str>,
        actor: Option<&str>,
    ) -> Result<OrdenExterna, ApiError> {
        self.avances.lock().await.push((
            order_id.to_string(),
            estado.to_string(),
            motivo.map(str::to_string),
            actor.map(str::to_string),
        ));
        let mut orden = self.orden.lock().await.clone().expect("orden");
        orden.estado = estado.to_string();
        Ok(orden)
    }
}

async fn provisionar(url: &str) {
    let pool = sqlx::PgPool::connect(url).await.expect("postgres");
    sqlx::raw_sql(
        r#"
        DROP SCHEMA IF EXISTS logistics CASCADE;
        CREATE SCHEMA logistics;
        CREATE TABLE logistics.envios (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          guia text NOT NULL UNIQUE,
          order_id uuid NOT NULL,
          monto_cents integer NOT NULL,
          estado text NOT NULL DEFAULT 'en_preparacion',
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          empresa_id uuid,
          proveedor_tipo text,
          proveedor_tenant_id uuid,
          parte_id uuid
        );
        CREATE UNIQUE INDEX idx_envios_orden_empresa
          ON logistics.envios (order_id, empresa_id);
        CREATE TABLE logistics.asignaciones_envio (
          id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
          envio_id uuid NOT NULL REFERENCES logistics.envios(id),
          proveedor_tipo_anterior text,
          proveedor_tipo_nuevo text NOT NULL,
          proveedor_tenant_id uuid NOT NULL,
          origen text NOT NULL,
          actor_usuario_id uuid,
          creado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE UNIQUE INDEX idx_asignacion_automatica_unica
          ON logistics.asignaciones_envio (envio_id) WHERE origen = 'automatica';
        CREATE TABLE logistics.eventos_procesados (
          event_id uuid PRIMARY KEY,
          tipo text NOT NULL,
          procesado_en timestamptz NOT NULL DEFAULT NOW()
        );
        CREATE TABLE logistics.outbox (
          event_id uuid PRIMARY KEY,
          tipo text NOT NULL,
          payload jsonb NOT NULL,
          estado text NOT NULL DEFAULT 'pendiente',
          creado_en timestamptz NOT NULL DEFAULT NOW(),
          publicado_en timestamptz
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("migracion logistics");
    pool.close().await;
}

fn capacidades(empresa: &str, rutas: bool, delivery: bool) -> Arc<dyn Capacidades> {
    let mut filas = Vec::new();
    if rutas {
        filas.push(CapacidadActiva {
            empresa_id: empresa.to_string(),
            capacidad_codigo: "logistica_rutas".to_string(),
            activo: true,
        });
    }
    if delivery {
        filas.push(CapacidadActiva {
            empresa_id: empresa.to_string(),
            capacidad_codigo: "fulfillment_delivery".to_string(),
            activo: true,
        });
    }
    Arc::new(CapacidadesFijas(filas))
}

fn orden(order_id: &str, empresas: &[&str]) -> OrdenExterna {
    OrdenExterna {
        id: order_id.to_string(),
        comprador_id: Uuid::new_v4().to_string(),
        parte_id: None,
        items: empresas
            .iter()
            .map(|empresa| OrdenItemExterna {
                empresa_id: (*empresa).to_string(),
                sku: "SKU-1".to_string(),
                cantidad: 1,
                producto_nombre: "Cafe".to_string(),
                precio_unitario_cents: 1_500,
            })
            .collect(),
        total: "30.00".to_string(),
        total_cents: 3_000,
        estado: "pagada".to_string(),
    }
}

#[tokio::test]
async fn debe_preparar_envio_idempotente_y_derivar_la_flota() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;

    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let orders = Arc::new(OrdersFake::default());
    let empresa = Uuid::new_v4().to_string();
    let envios = Envios::nueva(
        db.clone(),
        orders,
        capacidades(&empresa, true, true),
        "00000000-0000-0000-0000-000000000001",
        "logistics.outbox",
    );

    let primero = envios
        .preparar_envio(
            "00000000-0000-4000-8000-000000000001",
            &empresa,
            150_000,
            None,
        )
        .await
        .expect("preparar");
    assert!(primero.guia.starts_with("BGH-"));
    assert_eq!(primero.estado, "en_preparacion");
    assert_eq!(primero.monto_cents, 150_000);
    assert_eq!(primero.proveedor_tipo, None);

    let segundo = envios
        .preparar_envio(
            "00000000-0000-4000-8000-000000000001",
            &empresa,
            150_000,
            None,
        )
        .await
        .expect("idempotente");
    assert_eq!(primero.id, segundo.id);

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logistics.envios")
        .fetch_one(db.pool())
        .await
        .expect("conteo");
    assert_eq!(total, 1);

    let otra_empresa = Uuid::new_v4().to_string();
    let envios = Envios::nueva(
        db.clone(),
        Arc::new(OrdersFake::default()),
        capacidades(&otra_empresa, false, true),
        "00000000-0000-0000-0000-000000000001",
        "logistics.outbox",
    );
    let plataforma = envios
        .preparar_envio(
            "00000000-0000-4000-8000-000000000002",
            &otra_empresa,
            1_000,
            None,
        )
        .await
        .expect("plataforma");
    assert_eq!(plataforma.proveedor_tipo.as_deref(), Some("plataforma"));
    assert_eq!(
        plataforma.proveedor_tenant_id.as_deref(),
        Some("00000000-0000-0000-0000-000000000001")
    );
    let asignaciones: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logistics.asignaciones_envio")
        .fetch_one(db.pool())
        .await
        .expect("asignaciones");
    assert_eq!(asignaciones, 1);
}

#[tokio::test]
async fn debe_consumir_payment_procesado_y_emitir_shipment_started() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;

    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let empresa = Uuid::new_v4().to_string();
    let order_id = Uuid::new_v4().to_string();
    let orders = Arc::new(OrdersFake::default());
    *orders.orden.lock().await = Some(orden(&order_id, &[&empresa]));
    let envios = Arc::new(Envios::nueva(
        db.clone(),
        orders,
        capacidades(&empresa, true, false),
        "00000000-0000-0000-0000-000000000001",
        "logistics.outbox",
    ));
    let consumidor =
        LogisticsConsumer::nuevo(db.clone(), envios, Outbox::nueva("logistics.outbox"));

    let evento = json!({
        "event_id": Uuid::new_v4().to_string(),
        "tipo": "payment.procesado",
        "ocurrido_en": "2026-11-13T00:00:00.000Z",
        "data": {"order_id": order_id, "monto_cents": 1500, "estado": "procesado", "metodo": "simulado"}
    });
    consumidor.manejar(evento.clone()).await.expect("consumir");
    consumidor.manejar(evento.clone()).await.expect("replay");

    let eventos: Vec<(String, serde_json::Value)> =
        sqlx::query_as("SELECT tipo, payload FROM logistics.outbox ORDER BY creado_en")
            .fetch_all(db.pool())
            .await
            .expect("outbox");
    assert_eq!(eventos.len(), 1);
    assert_eq!(eventos[0].0, "shipment.started");
    assert_eq!(eventos[0].1["data"]["order_id"], order_id);
    assert_eq!(eventos[0].1["data"]["estado"], "en_preparacion");

    let procesados: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logistics.eventos_procesados")
        .fetch_one(db.pool())
        .await
        .expect("procesados");
    assert_eq!(procesados, 1);
}

#[tokio::test]
async fn debe_avanzar_la_orden_cuando_todos_los_envios_llegan_al_estado() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;

    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let order_id = Uuid::new_v4().to_string();
    let empresa_a = Uuid::new_v4().to_string();
    let empresa_b = Uuid::new_v4().to_string();
    let orders = Arc::new(OrdersFake::default());
    *orders.orden.lock().await = Some(orden(&order_id, &[&empresa_a, &empresa_b]));
    let envios = Envios::nueva(
        db.clone(),
        orders.clone(),
        capacidades(&empresa_a, true, true),
        "00000000-0000-0000-0000-000000000001",
        "logistics.outbox",
    );

    let a = envios
        .preparar_envio(&order_id, &empresa_a, 1_500, None)
        .await
        .expect("envio a");
    let b = envios
        .preparar_envio(&order_id, &empresa_b, 1_500, None)
        .await
        .expect("envio b");
    // Ambos envíos deben pertenecer a la flota que avanza.
    sqlx::query("UPDATE logistics.envios SET proveedor_tenant_id = $1, proveedor_tipo = 'empresa'")
        .bind(Uuid::parse_str(&empresa_a).expect("uuid"))
        .execute(db.pool())
        .await
        .expect("flota");

    envios
        .avanzar_estado_envio(
            &a.id,
            &empresa_a,
            "entregada",
            Some("recibido"),
            Some("u-l"),
        )
        .await
        .expect("avanzar a");
    assert!(orders.avances.lock().await.is_empty());

    envios
        .avanzar_estado_envio(
            &b.id,
            &empresa_a,
            "entregada",
            Some("recibido"),
            Some("u-l"),
        )
        .await
        .expect("avanzar b");
    let avances = orders.avances.lock().await;
    assert_eq!(avances.len(), 1);
    assert_eq!(avances[0].0, order_id);
    assert_eq!(avances[0].1, "entregada");
    assert_eq!(avances[0].2.as_deref(), Some("recibido"));
    assert_eq!(avances[0].3.as_deref(), Some("u-l"));
}

#[tokio::test]
async fn debe_rechazar_la_asignacion_de_un_envio_ya_fijado() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let _guard = DB_LOCK.lock().await;
    provisionar(&url).await;

    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let empresa = Uuid::new_v4().to_string();
    let envios = Envios::nueva(
        db.clone(),
        Arc::new(OrdersFake::default()),
        capacidades(&empresa, true, false),
        "00000000-0000-0000-0000-000000000001",
        "logistics.outbox",
    );
    let envio = envios
        .preparar_envio(
            "00000000-0000-4000-8000-000000000009",
            &empresa,
            1_000,
            None,
        )
        .await
        .expect("envio");

    let error = envios
        .asignar_proveedor(&envio.id, &empresa, "plataforma", None)
        .await
        .expect_err("asignacion fija");
    assert_eq!(error.codigo, "ASIGNACION_FIJA");
}
