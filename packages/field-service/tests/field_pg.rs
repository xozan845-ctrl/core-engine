//! Pruebas de integración contra PostgreSQL (solo si `TEST_DATABASE_URL` está
//! definida). El runner de CI las ejecuta con la BD efímera; en local se puede
//! apuntar a un contenedor desechable.

use async_trait::async_trait;
use field_service::clients::{Partes, PartesNoDisponible};
use field_service::db::Db;
use field_service::field::FieldService;
use serde_json::{json, Value};
use sqlx::PgPool;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use uuid::Uuid;

static MIGRACION: OnceLock<Mutex<()>> = OnceLock::new();

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

struct PartesPermitidas;
struct PartesDenegadas;

#[async_trait]
impl Partes for PartesPermitidas {
    async fn pertenece(&self, _: &str, _: &str) -> Result<bool, ()> {
        Ok(true)
    }
}

#[async_trait]
impl Partes for PartesDenegadas {
    async fn pertenece(&self, _: &str, _: &str) -> Result<bool, ()> {
        Ok(false)
    }
}

async fn preparar(url: &str) -> (Arc<Db>, PgPool) {
    let pool = PgPool::connect(url).await.expect("postgres");
    let _guard = MIGRACION.get_or_init(|| Mutex::new(())).lock().await;
    sqlx::raw_sql(include_str!("../../../infra/db/init/02_field.sql"))
        .execute(&pool)
        .await
        .expect("migracion field");
    // La migración 02 es anterior a `parte_id` (36_partes_compatibilidad.sql).
    // La FK a identity no aplica en esta prueba aislada, pero la columna sí.
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

fn servicio(db: Arc<Db>) -> FieldService {
    FieldService::nuevo(db, Arc::new(PartesNoDisponible))
}

#[tokio::test]
async fn debe_hacer_crud_y_borrado_logico_de_clientes() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let field = servicio(Arc::clone(&db));
    let tenant = Uuid::new_v4().to_string();

    let cliente = field
        .crear_cliente(
            &tenant,
            &json!({"nombreCompleto": "Ana", "email": "ana@test.dev"}),
        )
        .await
        .expect("crear cliente");
    assert_eq!(cliente["tenant_id"], tenant);
    assert_eq!(cliente["tipo_cliente"], "particular");
    assert_eq!(cliente["synced"], true);
    assert!(cliente["created_at"]
        .as_str()
        .expect("created_at")
        .ends_with('Z'));
    let id = cliente["id"].as_str().expect("id").to_string();

    let lista = field.listar_clientes(&tenant).await.expect("listar");
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0]["id"], id);

    let actualizado = field
        .actualizar_cliente(&tenant, &id, &json!({"telefono": "8888"}))
        .await
        .expect("actualizar");
    assert_eq!(actualizado["telefono"], "8888");
    assert_ne!(actualizado["updated_at"], cliente["updated_at"]);

    field
        .eliminar_cliente(&tenant, &id)
        .await
        .expect("eliminar");
    let error = field
        .obtener_cliente(&tenant, &id)
        .await
        .expect_err("borrado");
    assert_eq!(error.codigo, "NO_ENCONTRADO");
    assert_eq!(error.mensaje, format!("clientes {id} no existe."));

    let lista = field.listar_clientes(&tenant).await.expect("listar 2");
    assert!(lista.is_empty());

    db.cerrar().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_crear_rutas_con_paradas_ordenadas_y_filtrar_por_personal() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let field = servicio(Arc::clone(&db));
    let tenant = Uuid::new_v4().to_string();
    let personal = Uuid::new_v4().to_string();

    let vehiculo = field
        .crear_vehiculo(
            &tenant,
            &json!({
                "placa": "M123",
                "marca": "Toyota",
                "anio": 2026,
                "vencimientoSeguro": "2031-12-31"
            }),
        )
        .await
        .expect("vehiculo");
    assert_eq!(vehiculo["tipo"], "camioneta");
    assert_eq!(vehiculo["estado"], "disponible");
    // `date` se serializa como medianoche local (UTC-6), igual que node-postgres.
    assert_eq!(vehiculo["vencimiento_seguro"], "2031-12-31T06:00:00.000Z");

    let ruta = field
        .crear_ruta(
            &tenant,
            &json!({
                "nombre": "Ruta norte",
                "descripcion": "Reparto",
                "personalIds": [personal],
                "vehiculoAsignadoId": vehiculo["id"],
            }),
        )
        .await
        .expect("ruta");
    assert_eq!(ruta["estado"], "pendiente");
    assert_eq!(ruta["personal_asignado_ids"], json!([personal]));
    let ruta_id = ruta["id"].as_str().expect("id").to_string();

    let segunda = field
        .agregar_parada(&tenant, &ruta_id, &json!({"nombre": "B", "orden": 2}))
        .await
        .expect("parada 2");
    let primera = field
        .agregar_parada(
            &tenant,
            &ruta_id,
            &json!({"nombre": "A", "orden": 1, "rutaId": Uuid::new_v4()}),
        )
        .await
        .expect("parada 1");
    assert_eq!(primera["ruta_id"], ruta_id);
    assert_eq!(segunda["ruta_id"], ruta_id);

    let rutas = field
        .listar_rutas(&tenant, Some(&personal))
        .await
        .expect("listar rutas");
    assert_eq!(rutas.len(), 1);
    let paradas = rutas[0]["paradas"].as_array().expect("paradas");
    assert_eq!(paradas.len(), 2);
    assert_eq!(paradas[0]["nombre"], "A");
    assert_eq!(paradas[1]["nombre"], "B");

    let detalle = field
        .obtener_ruta(&tenant, &ruta_id)
        .await
        .expect("detalle");
    assert_eq!(detalle["paradas"].as_array().expect("paradas").len(), 2);

    let error = field
        .actualizar_parada(
            &tenant,
            &Uuid::new_v4().to_string(),
            segunda["id"].as_str().expect("id"),
            &json!({"nombre": "X"}),
        )
        .await
        .expect_err("otra ruta");
    assert_eq!(error.codigo, "PARADA_INVALIDA");

    db.cerrar().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_filtrar_pedidos_asistencia_tracking_e_incidencias() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let field = servicio(Arc::clone(&db));
    let tenant = Uuid::new_v4().to_string();
    let personal = Uuid::new_v4().to_string();
    let ruta = Uuid::new_v4().to_string();

    let pedido = field
        .crear_pedido(
            &tenant,
            &json!({"cliente": "Ana", "estado": "pendiente", "rutaId": ruta}),
        )
        .await
        .expect("pedido");
    field
        .crear_pedido(&tenant, &json!({"cliente": "Luis", "estado": "entregado"}))
        .await
        .expect("pedido 2");
    let pendientes = field
        .listar_pedidos(&tenant, Some("pendiente"), Some(&ruta), None)
        .await
        .expect("pedidos");
    assert_eq!(pendientes.len(), 1);
    assert_eq!(pendientes[0]["id"], pedido["id"]);

    let cambiado = field
        .cambiar_estado_pedido(
            &tenant,
            pedido["id"].as_str().expect("id"),
            &json!({"estado": "en_camino"}),
        )
        .await
        .expect("estado");
    assert_eq!(cambiado["estado"], "en_camino");

    field
        .registrar_asistencia(
            &tenant,
            &json!({
                "personalId": personal,
                "tipo": "entrada",
                "timestamp": "2026-10-14T13:00:00.000Z",
                "estadoPuntualidad": "puntual",
            }),
        )
        .await
        .expect("asistencia");
    let asistencia = field
        .listar_asistencia(&tenant, Some(&personal), Some("2026-10-14"))
        .await
        .expect("asistencia lista");
    assert_eq!(asistencia.len(), 1);
    assert_eq!(asistencia[0]["fotos_justificacion"], json!([]));

    field
        .registrar_tracking(
            &tenant,
            &[
                json!({"personalId": personal, "latitud": 12.1, "longitud": -86.2, "timestamp": "2026-10-14T13:00:00.000Z"}),
                json!({"personalId": personal, "latitud": 12.2, "longitud": -86.3, "timestamp": "2026-10-14T15:00:00.000Z"}),
            ],
        )
        .await
        .expect("tracking");
    let tracking = field
        .listar_tracking(
            &tenant,
            Some(&personal),
            Some("2026-10-14T14:00:00.000Z"),
            Some("2026-10-14T16:00:00.000Z"),
        )
        .await
        .expect("tracking lista");
    assert_eq!(tracking.len(), 1);
    assert_eq!(tracking[0]["latitud"], 12.2);

    let incidencia = field
        .crear_incidencia(
            &tenant,
            &json!({"tipo": "mecanica", "descripcion": "Falla", "rutaId": ruta}),
        )
        .await
        .expect("incidencia");
    assert_eq!(incidencia["estado"], "abierta");
    let incidencias = field
        .listar_incidencias(&tenant, Some("abierta"), Some(&ruta))
        .await
        .expect("incidencias");
    assert_eq!(incidencias.len(), 1);

    field
        .registrar_visita(
            &tenant,
            &json!({"personalId": personal, "tipoActividad": "entrega"}),
        )
        .await
        .expect("visita");
    let visitas = field
        .listar_visitas(&tenant, Some(&personal), None)
        .await
        .expect("visitas");
    assert_eq!(visitas.len(), 1);
    assert_eq!(visitas[0]["resultado"], "no_visitado");

    db.cerrar().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_hacer_upsert_de_cumplimiento_y_sync_offline() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let field = servicio(Arc::clone(&db));
    let tenant = Uuid::new_v4().to_string();
    let ruta = Uuid::new_v4().to_string();

    let primero = field
        .guardar_cumplimiento(
            &tenant,
            &json!({"rutaId": ruta, "fecha": "2026-10-14", "metricas": {"entregas": 8}}),
        )
        .await
        .expect("cumplimiento");
    assert_eq!(primero["metricas"]["entregas"], 8);
    let segundo = field
        .guardar_cumplimiento(
            &tenant,
            &json!({"rutaId": ruta, "fecha": "2026-10-14", "metricas": {"entregas": 9}}),
        )
        .await
        .expect("cumplimiento upsert");
    assert_eq!(segundo["id"], primero["id"]);
    assert_eq!(segundo["metricas"]["entregas"], 9);
    let consultado = field
        .obtener_cumplimiento(&tenant, &ruta, "2026-10-14")
        .await
        .expect("consultar");
    assert_eq!(consultado["metricas"]["entregas"], 9);

    let cliente_id = Uuid::new_v4().to_string();
    let resultados = field
        .sincronizar(
            &tenant,
            &[
                json!({"tipo": "desconocido", "payload": {"id": "x"}}),
                json!({"tipo": "cliente", "payload": {"id": cliente_id, "nombreCompleto": "Ana"}}),
                json!({"tipo": "cliente", "payload": {"id": cliente_id, "nombreCompleto": "Ana María"}}),
            ],
        )
        .await;
    assert_eq!(resultados.len(), 3);
    assert_eq!(resultados[0]["ok"], false);
    assert_eq!(resultados[0]["error"], "tipo de entidad no soportado");
    assert_eq!(resultados[1]["ok"], true);
    assert_eq!(resultados[2]["ok"], true);
    assert_eq!(resultados[2]["id"], cliente_id);

    let cliente = field
        .obtener_cliente(&tenant, &cliente_id)
        .await
        .expect("cliente sincronizado");
    assert_eq!(cliente["nombre_completo"], "Ana María");

    db.cerrar().await;
    pool.close().await;
}

#[tokio::test]
async fn debe_validar_la_parte_contra_identity() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let (db, pool) = preparar(&url).await;
    let tenant = Uuid::new_v4().to_string();
    let parte = Uuid::new_v4().to_string();

    let no_disponible = FieldService::nuevo(Arc::clone(&db), Arc::new(PartesNoDisponible));
    let error = no_disponible
        .crear_cliente(&tenant, &json!({"nombreCompleto": "Ana", "parteId": parte}))
        .await
        .expect_err("identity caida");
    assert_eq!(error.codigo, "SERVICIO_NO_DISPONIBLE");

    let denegadas = FieldService::nuevo(Arc::clone(&db), Arc::new(PartesDenegadas));
    let error = denegadas
        .crear_cliente(&tenant, &json!({"nombreCompleto": "Ana", "parteId": parte}))
        .await
        .expect_err("parte ajena");
    assert_eq!(error.codigo, "PARTE_INVALIDA");

    let permitidas = FieldService::nuevo(Arc::clone(&db), Arc::new(PartesPermitidas));
    let cliente = permitidas
        .crear_cliente(&tenant, &json!({"nombreCompleto": "Ana", "parteId": parte}))
        .await
        .expect("parte valida");
    assert_eq!(cliente["parte_id"], Value::String(parte));

    db.cerrar().await;
    pool.close().await;
}
