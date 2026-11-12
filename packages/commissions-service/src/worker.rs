//! Worker del corte quincenal (RN-07).
//!
//! Reemplaza `cron.schedule('5 0 1,15 * *')`: revisa cada 30 s la hora local
//! (`TZ`, por defecto `America/Managua`) y ejecuta el cierre una sola vez por
//! día 1 o 15 a las 00:05.

use crate::campos;
use crate::db::zona_local;
use crate::liquidaciones::LiquidacionesPort;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use chrono::{DateTime, Datelike, NaiveDate, Timelike, Utc};
use chrono_tz::Tz;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

pub const INTERVALO_REVISION: Duration = Duration::from_secs(30);

/// `true` si en este minuto toca el corte y aún no se ejecutó hoy.
pub fn debe_ejecutar(ahora: DateTime<Tz>, ultima: Option<NaiveDate>) -> bool {
    ahora.hour() == 0
        && ahora.minute() == 5
        && matches!(ahora.day(), 1 | 15)
        && ultima != Some(ahora.date_naive())
}

pub fn iniciar_worker(liquidaciones: Arc<dyn LiquidacionesPort>) -> JoinHandle<()> {
    tokio::spawn(async move {
        crate::logs::info(
            "commissions.liquidacion",
            campos! { "msg": "Planificador de liquidacion quincenal activo (dias 1 y 15)" },
        );
        let mut ultima: Option<NaiveDate> = None;
        let mut ticker = tokio::time::interval(INTERVALO_REVISION);
        loop {
            ticker.tick().await;
            let ahora = Utc::now().with_timezone(&zona_local());
            if !debe_ejecutar(ahora, ultima) {
                continue;
            }
            ultima = Some(ahora.date_naive());
            let contexto = ContextoCorrelacion {
                request_id: format!("liquidaciones-worker-{}", Utc::now().timestamp_millis()),
                servicio: "liquidaciones-worker".to_string(),
                usuario_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
                rol: Some("super_admin".to_string()),
                tenant_id: None,
            };
            let resultado = ejecutar_con_contexto(
                contexto,
                liquidaciones.cerrar_periodo_anterior(ahora.date_naive()),
            )
            .await;
            if let Err(error) = resultado {
                crate::logs::error(
                    "commissions.liquidacion",
                    campos! { "msg": "Fallo el corte quincenal", "err": error.to_string() },
                );
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn en_managua(anio: i32, mes: u32, dia: u32, hora: u32, minuto: u32) -> DateTime<Tz> {
        chrono_tz::America::Managua
            .with_ymd_and_hms(anio, mes, dia, hora, minuto, 0)
            .single()
            .expect("fecha local")
    }

    #[test]
    fn debe_ejecutar_el_dia_1_y_15_a_las_0005() {
        assert!(debe_ejecutar(en_managua(2026, 9, 1, 0, 5), None));
        assert!(debe_ejecutar(en_managua(2026, 9, 15, 0, 5), None));
    }

    #[test]
    fn no_debe_ejecutar_otro_dia_u_otra_hora() {
        assert!(!debe_ejecutar(en_managua(2026, 9, 2, 0, 5), None));
        assert!(!debe_ejecutar(en_managua(2026, 9, 1, 0, 6), None));
        assert!(!debe_ejecutar(en_managua(2026, 9, 1, 1, 5), None));
    }

    #[test]
    fn no_debe_repetir_el_corte_del_mismo_dia() {
        let ahora = en_managua(2026, 9, 15, 0, 5);
        assert!(!debe_ejecutar(ahora, Some(ahora.date_naive())));
    }
}
