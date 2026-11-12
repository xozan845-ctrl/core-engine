//! Pruebas de integración contra PostgreSQL (solo si `TEST_DATABASE_URL` está
//! definida y el esquema `scheduling` existe). El runner de CI las ejecuta con
//! la BD efímera; en local se puede apuntar a un contenedor desechable.

use async_trait::async_trait;
use scheduling_service::clients::Partes;
use scheduling_service::db::Db;
use scheduling_service::scheduling::{
    Agenda, CrearBloqueo, CrearCita, CrearDisponibilidad, CrearRecurso, OrdenItem, OrdenReserva,
    VerificarDisponibilidad,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

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

#[tokio::test]
async fn debe_crear_disponibilidad_y_bloqueos_con_las_mismas_reglas() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    // La prueba provisiona su propio esquema para poder correr en CI sin psql.
    let pool = sqlx::PgPool::connect(&url).await.expect("postgres");
    sqlx::raw_sql(include_str!("../../../../infra/db/init/28_scheduling.sql"))
        .execute(&pool)
        .await
        .expect("migracion scheduling");
    sqlx::query("ALTER TABLE scheduling.citas ADD COLUMN IF NOT EXISTS parte_id uuid")
        .execute(&pool)
        .await
        .expect("compatibilidad parte_id");
    pool.close().await;

    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let agenda = Agenda::nueva(db.clone(), Arc::new(PartesPermitidas), "scheduling.outbox");
    let empresa = Uuid::new_v4().to_string();
    let empresa_uuid = Uuid::parse_str(&empresa).expect("uuid");

    let recurso = agenda
        .crear_recurso(
            &empresa,
            &CrearRecurso {
                tipo: "sala".to_string(),
                nombre: format!("Sala {}", &empresa[..8]),
                atributos: json!({ "piso": 2 }),
            },
        )
        .await
        .expect("crear recurso");
    let recurso_id = recurso["id"].as_str().expect("id").to_string();
    assert_eq!(recurso["estado"], "activo");
    assert_eq!(recurso["atributos"], json!({ "piso": 2 }));
    assert!(recurso["creado_en"]
        .as_str()
        .expect("creado_en")
        .ends_with('Z'));

    let lista = agenda.listar_recursos(&empresa).await.expect("listar");
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0]["id"], recurso_id);

    let disponibilidad = agenda
        .crear_disponibilidad(
            &empresa,
            &CrearDisponibilidad {
                recurso_id: recurso_id.clone(),
                dia_semana: 1,
                inicia_hora: "08:00".to_string(),
                termina_hora: "18:00".to_string(),
                zona_horaria: Some("America/Managua".to_string()),
                vigente_desde: None,
                vigente_hasta: None,
            },
        )
        .await
        .expect("crear disponibilidad");
    assert_eq!(disponibilidad["inicia_hora"], "08:00:00");
    assert_eq!(disponibilidad["dia_semana"], 1);
    // `date` se serializa como medianoche local (UTC-6), igual que node-postgres.
    assert!(disponibilidad["vigente_desde"]
        .as_str()
        .expect("vigente_desde")
        .ends_with("T06:00:00.000Z"));

    let ventana = VerificarDisponibilidad {
        empresa_id: empresa.clone(),
        recurso_id: recurso_id.clone(),
        inicia_en: "2032-04-12T14:00:00.000Z".to_string(),
        termina_en: "2032-04-12T15:00:00.000Z".to_string(),
    };
    agenda
        .verificar_disponibilidad(&ventana)
        .await
        .expect("disponible");

    let bloqueo = agenda
        .crear_bloqueo(
            &empresa,
            None,
            &CrearBloqueo {
                recurso_id: recurso_id.clone(),
                inicia_en: ventana.inicia_en.clone(),
                termina_en: ventana.termina_en.clone(),
                motivo: Some("mantenimiento".to_string()),
            },
        )
        .await
        .expect("crear bloqueo");
    assert_eq!(bloqueo["motivo"], "mantenimiento");

    let bloqueado = agenda
        .verificar_disponibilidad(&ventana)
        .await
        .expect_err("bloqueado");
    assert_eq!(bloqueado.codigo, "RECURSO_NO_DISPONIBLE");
    assert_eq!(bloqueado.status.as_u16(), 409);

    let solapado = agenda
        .crear_bloqueo(
            &empresa,
            None,
            &CrearBloqueo {
                recurso_id: recurso_id.clone(),
                inicia_en: "2032-04-12T14:30:00.000Z".to_string(),
                termina_en: "2032-04-12T15:30:00.000Z".to_string(),
                motivo: None,
            },
        )
        .await
        .expect_err("conflicto GiST");
    assert_eq!(solapado.codigo, "RECURSO_NO_DISPONIBLE");
    assert_eq!(solapado.status.as_u16(), 409);

    let fuera_de_ventana = agenda
        .verificar_disponibilidad(&VerificarDisponibilidad {
            empresa_id: empresa.clone(),
            recurso_id: recurso_id.clone(),
            inicia_en: "2032-04-12T03:00:00.000Z".to_string(),
            termina_en: "2032-04-12T04:00:00.000Z".to_string(),
        })
        .await
        .expect_err("fuera de ventana");
    assert_eq!(fuera_de_ventana.codigo, "RECURSO_NO_DISPONIBLE");

    let listado = agenda
        .listar_disponibilidad(&empresa, &recurso_id)
        .await
        .expect("listar disponibilidad");
    assert_eq!(listado.len(), 1);

    // Citas: ventana 10:00-11:00 local (16:00-17:00Z) para no chocar con el bloqueo.
    let parte = Uuid::new_v4().to_string();
    let cita = agenda
        .crear_cita(
            &empresa,
            &CrearCita {
                recurso_id: recurso_id.clone(),
                parte_id: parte.clone(),
                order_id: None,
                item_id: None,
                inicia_en: "2032-04-12T16:00:00.000Z".to_string(),
                termina_en: "2032-04-12T17:00:00.000Z".to_string(),
                notas: Some("primera visita".to_string()),
                metadata: json!({ "canal": "web" }),
            },
        )
        .await
        .expect("crear cita");
    assert_eq!(cita["estado"], "reservada");
    assert_eq!(cita["parte_id"], parte);
    assert_eq!(cita["notas"], "primera visita");
    assert_eq!(cita["metadata"]["canal"], "web");

    let citas = agenda
        .listar_citas(&empresa, None, None)
        .await
        .expect("citas");
    assert_eq!(citas.len(), 1);
    let recordatorios = agenda
        .listar_recordatorios(&empresa)
        .await
        .expect("recordatorios");
    assert_eq!(recordatorios.len(), 2);
    assert_eq!(recordatorios[0]["estado"], "pendiente");

    let outbox = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM scheduling.outbox WHERE payload->'data'->>'empresa_id' = $1",
    )
    .bind(&empresa)
    .fetch_one(db.pool())
    .await
    .expect("outbox");
    assert_eq!(outbox, 1);
    let tipo = sqlx::query_scalar::<_, String>(
        "SELECT tipo FROM scheduling.outbox WHERE payload->'data'->>'empresa_id' = $1",
    )
    .bind(&empresa)
    .fetch_one(db.pool())
    .await
    .expect("tipo outbox");
    assert_eq!(tipo, "appointment.created");

    // Worker: un recordatorio vencido se marca enviado y publica su evento.
    let cita_uuid = Uuid::parse_str(cita["id"].as_str().expect("id")).expect("uuid cita");
    sqlx::query(
        "UPDATE scheduling.recordatorios SET programado_en = NOW() - INTERVAL '1 minute'
         WHERE cita_id = $1 AND tipo = '2h'",
    )
    .bind(cita_uuid)
    .execute(db.pool())
    .await
    .expect("recordatorio vencido");
    assert_eq!(agenda.procesar_recordatorios().await.expect("procesar"), 1);
    let tipo_recordatorio: String = sqlx::query_scalar(
        "SELECT tipo FROM scheduling.outbox WHERE tipo = 'appointment.reminder.due'",
    )
    .fetch_one(db.pool())
    .await
    .expect("outbox recordatorio");
    assert_eq!(tipo_recordatorio, "appointment.reminder.due");

    // Reserva desde `order.created` + idempotencia por order_id.
    let orden = OrdenReserva {
        order_id: Uuid::new_v4().to_string(),
        cliente_parte_id: parte.clone(),
        items: vec![OrdenItem {
            empresa_id: empresa.clone(),
            item_type: "servicio".to_string(),
            item_id: Some(Uuid::new_v4().to_string()),
            fulfillment: Some("onsite".to_string()),
            fulfillment_config: Some(json!({
                "recurso_id": recurso_id,
                "inicia_en": "2032-04-12T17:00:00.000Z",
                "termina_en": "2032-04-12T18:00:00.000Z",
                "notas": "reserva"
            })),
        }],
    };
    let creada = agenda
        .crear_desde_reserva(&orden)
        .await
        .expect("crear desde reserva");
    assert_eq!(creada["order_id"], orden.order_id);
    assert_eq!(creada["notas"], "reserva");
    let repetida = agenda
        .crear_desde_reserva(&orden)
        .await
        .expect("idempotente");
    assert_eq!(repetida["id"], creada["id"]);

    let sin_config = agenda
        .crear_desde_reserva(&OrdenReserva {
            order_id: Uuid::new_v4().to_string(),
            cliente_parte_id: parte.clone(),
            items: vec![],
        })
        .await
        .expect_err("configuracion requerida");
    assert_eq!(sin_config.codigo, "CITA_CONFIGURACION_REQUERIDA");

    let cancelada = agenda
        .cancelar_cita(&empresa, cita["id"].as_str().expect("id"), Some("cambio"))
        .await
        .expect("cancelar");
    assert_eq!(cancelada["estado"], "cancelada");
    let pendientes = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM scheduling.recordatorios
         WHERE cita_id = $1 AND estado = 'pendiente'",
    )
    .bind(cita_uuid)
    .fetch_one(db.pool())
    .await
    .expect("pendientes");
    assert_eq!(pendientes, 0);

    // Limpieza (R-I-9): cada prueba deja la BD como la encontró.
    for tabla in [
        "recordatorios",
        "citas",
        "bloqueos",
        "disponibilidad",
        "recursos",
    ] {
        sqlx::query(&format!(
            "DELETE FROM scheduling.{tabla} WHERE empresa_id = $1"
        ))
        .bind(empresa_uuid)
        .execute(db.pool())
        .await
        .expect("limpieza");
    }
    sqlx::query("DELETE FROM scheduling.outbox WHERE payload->'data'->>'empresa_id' = $1")
        .bind(&empresa)
        .execute(db.pool())
        .await
        .expect("limpieza outbox");
}
