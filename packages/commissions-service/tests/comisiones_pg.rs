//! Pruebas de integración contra PostgreSQL (solo si `TEST_DATABASE_URL` está
//! definida). El runner de CI las ejecuta con la BD efímera.

use async_trait::async_trait;
use commissions_service::clients::{ItemOrden, KpisNoDisponible, OrdenInterna, Ordenes};
use commissions_service::comisiones::{Comisiones, DatosLinea};
use commissions_service::db::Db;
use commissions_service::liquidaciones::{Liquidaciones, LiquidacionesPort, Periodo};
use commissions_service::outbox::Outbox;
use commissions_service::pedidos::PedidosConsumer;
use commissions_service::reportes::ReportesPort;
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use uuid::Uuid;

static MIGRACION: OnceLock<Mutex<()>> = OnceLock::new();

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

async fn preparar() -> Option<(Arc<Db>, PgPool)> {
    let url = url()?;
    let pool = PgPool::connect(&url).await.expect("postgres");
    let _guard = MIGRACION.get_or_init(|| Mutex::new(())).lock().await;
    sqlx::raw_sql(include_str!("sql/commissions.sql"))
        .execute(&pool)
        .await
        .expect("migracion commissions");
    let db = Arc::new(Db::conectar(&url).await.expect("pool del servicio"));
    Some((db, pool))
}

struct OrdenFija {
    orden: OrdenInterna,
}

#[async_trait]
impl Ordenes for OrdenFija {
    async fn obtener(&self, _order_id: &str) -> Option<OrdenInterna> {
        Some(self.orden.clone())
    }
}

fn item(empresa: Uuid, tienda: Uuid, vendedor: Uuid, oferta: Uuid) -> ItemOrden {
    ItemOrden {
        oferta_id: oferta.to_string(),
        vendedor_id: Some(vendedor.to_string()),
        empresa_id: Some(empresa.to_string()),
        tienda_id: Some(tienda.to_string()),
        participacion_porcentaje: Some(60),
        precio_unitario_cents: 115_000,
        cantidad: 1,
    }
}

#[tokio::test]
async fn debe_acreditar_una_linea_una_sola_vez_y_con_los_montos_de_rn04() {
    let Some((db, pool)) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integracion");
        return;
    };
    let comisiones = Comisiones::nueva(Arc::clone(&db), 0.12);
    let order_id = Uuid::new_v4().to_string();
    let datos = DatosLinea {
        order_id: order_id.clone(),
        linea_clave: "of-1".into(),
        empresa_id: Uuid::new_v4().to_string(),
        tienda_id: Uuid::new_v4().to_string(),
        vendedor_id: Uuid::new_v4().to_string(),
        venta_cents: 115_000,
        participacion: 60,
    };
    let mut tx = db.transaccion().await.expect("tx");
    let primera = comisiones
        .acreditar_linea(&mut tx, &datos)
        .await
        .expect("acreditar")
        .expect("primera");
    tx.commit().await.expect("commit");
    assert_eq!(primera.comision_cents, 13_800);
    assert_eq!(primera.monto_vendedor_cents, 60_720);
    assert_eq!(primera.monto_empresario_cents, Some(40_480));
    assert_eq!(primera.estado, "acreditada");
    assert!(primera.creado_en.ends_with('Z'));

    let mut tx = db.transaccion().await.expect("tx");
    let segunda = comisiones
        .acreditar_linea(&mut tx, &datos)
        .await
        .expect("acreditar de nuevo");
    tx.commit().await.expect("commit");
    assert!(segunda.is_none(), "idempotencia por orden y linea");

    let filas: Vec<(Uuid, i32)> =
        sqlx::query_as("SELECT id, comision_cents FROM commissions.comisiones WHERE order_id = $1")
            .bind(Uuid::parse_str(&order_id).expect("uuid"))
            .fetch_all(&pool)
            .await
            .expect("consulta");
    assert_eq!(filas.len(), 1);
    assert_eq!(filas[0].1, 13_800);

    db.pool().close().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_compensar_una_devolucion_una_sola_vez() {
    let Some((db, pool)) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integracion");
        return;
    };
    let comisiones = Comisiones::nueva(Arc::clone(&db), 0.12);
    let order_id = Uuid::new_v4().to_string();
    let empresa = Uuid::new_v4();
    let datos = DatosLinea {
        order_id: order_id.clone(),
        linea_clave: "of-2".into(),
        empresa_id: empresa.to_string(),
        tienda_id: Uuid::new_v4().to_string(),
        vendedor_id: Uuid::new_v4().to_string(),
        venta_cents: 10_000,
        participacion: 50,
    };
    let mut tx = db.transaccion().await.expect("tx");
    comisiones
        .acreditar_linea(&mut tx, &datos)
        .await
        .expect("acreditar")
        .expect("comision");
    comisiones
        .compensar(&mut tx, &order_id, "devolucion aprobada")
        .await
        .expect("compensar");
    tx.commit().await.expect("commit");

    let mut tx = db.transaccion().await.expect("tx");
    comisiones
        .compensar(&mut tx, &order_id, "devolucion aprobada")
        .await
        .expect("compensar de nuevo");
    tx.commit().await.expect("commit");

    let filas: Vec<(i32, i32, String)> = sqlx::query_as(
        "SELECT monto_cents, monto_empresario_cents, motivo
         FROM commissions.compensaciones_devoluciones WHERE order_id = $1",
    )
    .bind(Uuid::parse_str(&order_id).expect("uuid"))
    .fetch_all(&pool)
    .await
    .expect("compensaciones");
    assert_eq!(filas.len(), 1, "una devolucion no duplica la compensacion");
    assert_eq!(filas[0].0, -4_400);
    assert_eq!(filas[0].1, -4_400);
    assert_eq!(filas[0].2, "devolucion aprobada");

    let estado: (String,) =
        sqlx::query_as("SELECT estado FROM commissions.comisiones WHERE order_id = $1")
            .bind(Uuid::parse_str(&order_id).expect("uuid"))
            .fetch_one(&pool)
            .await
            .expect("estado");
    assert_eq!(estado.0, "compensada");

    db.pool().close().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_registrar_el_pago_una_sola_vez_por_orden() {
    let Some((db, pool)) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integracion");
        return;
    };
    let comisiones = Comisiones::nueva(Arc::clone(&db), 0.12);
    let order_id = Uuid::new_v4().to_string();
    let mut tx = db.transaccion().await.expect("tx");
    let primero = comisiones
        .registrar_pago(&mut tx, &order_id, 2_500)
        .await
        .expect("pago");
    let segundo = comisiones
        .registrar_pago(&mut tx, &order_id, 9_999)
        .await
        .expect("pago repetido");
    tx.commit().await.expect("commit");
    assert_eq!(primero.id, segundo.id);
    assert_eq!(segundo.monto_cents, 2_500);
    assert_eq!(segundo.estado, "procesado");
    assert_eq!(segundo.metodo, "simulado");
    let total: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM commissions.pagos WHERE order_id = $1")
            .bind(Uuid::parse_str(&order_id).expect("uuid"))
            .fetch_one(&pool)
            .await
            .expect("conteo");
    assert_eq!(total.0, 1);

    db.pool().close().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_consolidar_el_corte_quincenal_por_vendedor_y_empresa() {
    let Some((db, pool)) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integracion");
        return;
    };
    let v1 = Uuid::new_v4();
    let v2 = Uuid::new_v4();
    let e1 = Uuid::new_v4();
    let e2 = Uuid::new_v4();
    // Periodo aleatorio para aislar la prueba de ejecuciones anteriores.
    let entropia = Uuid::new_v4().as_u128();
    let anio = 2040 + (entropia % 30) as i32;
    let mes = 1 + ((entropia >> 8) % 12) as u32;
    let fecha = format!("{anio:04}-{mes:02}-10");
    let inicio = format!("{anio:04}-{mes:02}-01");
    let fin = format!("{anio:04}-{mes:02}-15");
    for (vendedor, empresa, venta, comision, mv, me) in [
        (v1, e1, 10_000_i32, 1_200_i32, 5_280_i32, 3_520_i32),
        (v1, e1, 5_000, 600, 2_640, 1_760),
        (v2, e2, 20_000, 2_400, 10_560, 7_040),
    ] {
        sqlx::query(
            "INSERT INTO commissions.comisiones
                (id, order_id, linea_clave, empresa_id, tienda_id, vendedor_id, venta_cents,
                 comision_cents, monto_vendedor_cents, monto_empresario_cents,
                 participacion_porcentaje, estado, creado_en)
             VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9, 60, 'acreditada', $10::date)",
        )
        .bind(Uuid::new_v4())
        .bind(format!("of-{}", Uuid::new_v4()))
        .bind(empresa)
        .bind(Uuid::new_v4())
        .bind(vendedor)
        .bind(venta)
        .bind(comision)
        .bind(mv)
        .bind(me)
        .bind(&fecha)
        .execute(&pool)
        .await
        .expect("comision");
    }
    sqlx::query(
        "INSERT INTO commissions.compensaciones_devoluciones
            (id, comision_id, order_id, vendedor_id, empresa_id, monto_cents,
             monto_empresario_cents, motivo, creado_en)
         VALUES (gen_random_uuid(), NULL, $1, $2, $3, -100, -20, 'devolucion', $4::date)",
    )
    .bind(Uuid::new_v4())
    .bind(v1)
    .bind(e1)
    .bind(format!("{anio:04}-{mes:02}-12"))
    .execute(&pool)
    .await
    .expect("compensacion");

    let liquidaciones = Liquidaciones::nueva(Arc::clone(&db));
    let filas = liquidaciones
        .cerrar_periodo(Periodo {
            inicio: inicio.clone(),
            fin: fin.clone(),
        })
        .await
        .expect("corte");
    assert_eq!(filas.len(), 2);
    let mut por_vendedor: Vec<(String, i64)> = filas
        .iter()
        .map(|fila| (fila.vendedor_id.clone(), fila.monto_cents))
        .collect();
    por_vendedor.sort();
    let mut esperado = vec![
        (v1.to_string(), 5_280 + 2_640 - 100),
        (v2.to_string(), 10_560),
    ];
    esperado.sort();
    assert_eq!(por_vendedor, esperado);
    assert!(filas
        .iter()
        .all(|fila| fila.periodo_inicio.starts_with(&inicio)));

    let empresas: Vec<(i32,)> = sqlx::query_as(
        "SELECT monto_cents FROM commissions.liquidaciones_empresas
         WHERE empresa_id = ANY($1) ORDER BY monto_cents",
    )
    .bind(vec![e1, e2])
    .fetch_all(&pool)
    .await
    .expect("liquidaciones empresa");
    assert_eq!(empresas, vec![(1_760 + 3_520 - 20,), (7_040,)]);

    let pagada = liquidaciones
        .marcar_pagada(&filas[0].id)
        .await
        .expect("pagar")
        .expect("liquidacion");
    assert_eq!(pagada.estado, "pagada");

    db.pool().close().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_procesar_el_evento_de_orden_completada_una_sola_vez() {
    let Some((db, pool)) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integracion");
        return;
    };
    let order_id = Uuid::new_v4();
    let orden = OrdenInterna {
        total_cents: 115_000,
        items: vec![item(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )],
    };
    let comisiones = Arc::new(Comisiones::nueva(Arc::clone(&db), 0.12));
    let consumidor = PedidosConsumer::nuevo(
        Arc::clone(&db),
        Arc::clone(&comisiones),
        Outbox::nueva("commissions.outbox"),
        Arc::new(OrdenFija { orden }),
    );
    let evento = serde_json::json!({
        "event_id": Uuid::new_v4().to_string(),
        "tipo": "order.completado",
        "ocurrido_en": "2026-09-01T00:00:00.000Z",
        "data": { "order_id": order_id.to_string(), "entregado_en": "2026-09-01T00:00:00.000Z" }
    });
    consumidor.manejar(evento.clone()).await.expect("primera");
    consumidor.manejar(evento).await.expect("segunda");

    let comisiones_filas: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM commissions.comisiones WHERE order_id = $1")
            .bind(order_id)
            .fetch_one(&pool)
            .await
            .expect("conteo comisiones");
    assert_eq!(comisiones_filas.0, 1);
    let outbox: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM commissions.outbox
         WHERE tipo = 'comision.acreditada' AND payload->'data'->>'order_id' = $1",
    )
    .bind(order_id.to_string())
    .fetch_one(&pool)
    .await
    .expect("conteo outbox");
    assert_eq!(outbox.0, 1);

    db.pool().close().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_registrar_pago_y_compensar_por_eventos() {
    let Some((db, pool)) = preparar().await else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integracion");
        return;
    };
    let order_id = Uuid::new_v4();
    let orden = OrdenInterna {
        total_cents: 10_000,
        items: vec![item(
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )],
    };
    let comisiones = Arc::new(Comisiones::nueva(Arc::clone(&db), 0.12));
    let consumidor = PedidosConsumer::nuevo(
        Arc::clone(&db),
        comisiones,
        Outbox::nueva("commissions.outbox"),
        Arc::new(OrdenFija { orden }),
    );
    let pago_id = Uuid::new_v4();
    let devolucion_id = Uuid::new_v4();
    let pago = serde_json::json!({
        "event_id": pago_id.to_string(),
        "tipo": "payment.procesado",
        "data": { "order_id": order_id.to_string(), "monto_cents": 10_000 }
    });
    let devolucion = serde_json::json!({
        "event_id": devolucion_id.to_string(),
        "tipo": "devolucion.solicitada",
        "data": { "order_id": order_id.to_string(), "motivo": "cliente devuelve" }
    });
    consumidor.manejar(pago).await.expect("pago");
    consumidor.manejar(devolucion).await.expect("devolucion");
    let pagos: (i64,) =
        sqlx::query_as("SELECT COUNT(*)::bigint FROM commissions.pagos WHERE order_id = $1")
            .bind(order_id)
            .fetch_one(&pool)
            .await
            .expect("pagos");
    assert_eq!(pagos.0, 1);
    let eventos: (i64,) = sqlx::query_as(
        "SELECT COUNT(*)::bigint FROM commissions.eventos_procesados WHERE event_id = ANY($1)",
    )
    .bind(vec![pago_id, devolucion_id])
    .fetch_one(&pool)
    .await
    .expect("eventos");
    assert_eq!(eventos.0, 2);

    let reportes =
        commissions_service::reportes::Reportes::nueva(Arc::clone(&db), Arc::new(KpisNoDisponible));
    let kpis = reportes.kpis().await.expect("kpis");
    assert!(kpis.gmv_mes_cents >= 0);
    assert!(kpis.pedidos_mes >= 1);

    db.pool().close().await;
    pool.close().await;
}
