//! Pruebas de integración contra PostgreSQL (solo si `TEST_DATABASE_URL` está
//! definida y el esquema `scheduling` existe). El runner de CI las ejecuta con
//! la BD efímera; en local se puede apuntar a un contenedor desechable.

use scheduling_service::db::Db;
use scheduling_service::scheduling::{
    Agenda, CrearBloqueo, CrearDisponibilidad, CrearRecurso, VerificarDisponibilidad,
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

fn url() -> Option<String> {
    std::env::var("TEST_DATABASE_URL").ok()
}

#[tokio::test]
async fn debe_crear_disponibilidad_y_bloqueos_con_las_mismas_reglas() {
    let Some(url) = url() else {
        eprintln!("TEST_DATABASE_URL no definida: se omite la integración");
        return;
    };
    let db = Arc::new(Db::conectar(&url).await.expect("postgres"));
    let agenda = Agenda::nueva(db.clone());
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

    // Limpieza (R-I-9): cada prueba deja la BD como la encontró.
    for tabla in ["bloqueos", "disponibilidad", "recursos"] {
        sqlx::query(&format!(
            "DELETE FROM scheduling.{tabla} WHERE empresa_id = $1"
        ))
        .bind(empresa_uuid)
        .execute(db.pool())
        .await
        .expect("limpieza");
    }
}
