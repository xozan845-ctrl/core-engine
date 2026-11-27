//! Worker de dispersión diaria.
//!
//! A la hora configurada (por defecto 00:10 local) cierra el día anterior,
//! crea los payouts por beneficiario y, si la dispersión está activa, los envía
//! al proveedor configurado. Es idempotente: reintenta sin duplicar payouts.

use crate::campos;
use crate::db::zona_local;
use crate::dispersion::Dispersion;
use crate::logs::{ejecutar_con_contexto, ContextoCorrelacion};
use chrono::{DateTime, NaiveDate, Timelike, Utc};
use chrono_tz::Tz;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

pub const INTERVALO_REVISION: Duration = Duration::from_secs(30);

/// `true` si en este minuto toca el cierre diario y aún no se ejecutó hoy.
pub fn debe_ejecutar(ahora: DateTime<Tz>, hora_local: &str, ultima: Option<NaiveDate>) -> bool {
    let Some((hora, minuto)) = hora_local.split_once(':') else {
        return false;
    };
    let (Ok(hora), Ok(minuto)) = (hora.parse::<u32>(), minuto.parse::<u32>()) else {
        return false;
    };
    ahora.hour() == hora && ahora.minute() == minuto && ultima != Some(ahora.date_naive())
}

pub fn iniciar_worker(dispersion: Arc<Dispersion>) -> JoinHandle<()> {
    tokio::spawn(async move {
        crate::logs::info(
            "commissions.dispersion",
            campos! { "msg": "Planificador de dispersion diaria activo" },
        );
        let mut ultima: Option<NaiveDate> = None;
        let mut ticker = tokio::time::interval(INTERVALO_REVISION);
        loop {
            ticker.tick().await;
            let config = match dispersion.configuracion().await {
                Ok(config) => config,
                Err(error) => {
                    crate::logs::error(
                        "commissions.dispersion",
                        campos! { "msg": "No se pudo leer la configuracion de dispersion", "err": error.to_string() },
                    );
                    continue;
                }
            };
            let ahora = Utc::now().with_timezone(&zona_local());
            if !debe_ejecutar(ahora, &config.hora_local, ultima) {
                continue;
            }
            ultima = Some(ahora.date_naive());
            let fecha = ahora
                .date_naive()
                .pred_opt()
                .unwrap_or_else(|| ahora.date_naive());
            let contexto = ContextoCorrelacion {
                request_id: format!("dispersion-worker-{}", Utc::now().timestamp_millis()),
                servicio: "dispersion-worker".to_string(),
                usuario_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
                rol: Some("super_admin".to_string()),
                tenant_id: None,
                negocio_id: None,
            };
            match ejecutar_con_contexto(contexto, dispersion.cerrar_dia(fecha)).await {
                Ok(resumen) => {
                    crate::logs::info(
                        "commissions.dispersion",
                        campos! {
                            "msg": "Cierre diario ejecutado",
                            "fecha": fecha.format("%Y-%m-%d").to_string(),
                            "liquidaciones": resumen.liquidaciones_cerradas,
                            "payouts": resumen.payouts_creados,
                        },
                    );
                }
                Err(error) => {
                    crate::logs::error(
                        "commissions.dispersion",
                        campos! { "msg": "Fallo el cierre diario", "err": error.to_string() },
                    );
                    continue;
                }
            }
            if config.activo {
                let contexto = ContextoCorrelacion {
                    request_id: format!("dispersion-envio-{}", Utc::now().timestamp_millis()),
                    servicio: "dispersion-worker".to_string(),
                    usuario_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
                    rol: Some("super_admin".to_string()),
                    tenant_id: None,
                    negocio_id: None,
                };
                match ejecutar_con_contexto(contexto, dispersion.enviar_pendientes()).await {
                    Ok(resumen) => {
                        crate::logs::info(
                            "commissions.dispersion",
                            campos! {
                                "msg": "Envio diario ejecutado",
                                "enviados": resumen.payouts_enviados,
                                "fallidos": resumen.payouts_fallidos,
                            },
                        );
                    }
                    Err(error) => {
                        crate::logs::error(
                            "commissions.dispersion",
                            campos! { "msg": "Fallo el envio diario", "err": error.to_string() },
                        );
                    }
                }
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
    fn debe_ejecutar_todos_los_dias_a_la_hora_configurada() {
        assert!(debe_ejecutar(en_managua(2026, 9, 1, 0, 10), "00:10", None));
        assert!(debe_ejecutar(en_managua(2026, 9, 2, 0, 10), "00:10", None));
    }

    #[test]
    fn no_debe_ejecutar_fuera_de_la_hora_o_dos_veces_el_mismo_dia() {
        let ahora = en_managua(2026, 9, 1, 0, 10);
        assert!(!debe_ejecutar(ahora, "00:11", None));
        assert!(!debe_ejecutar(ahora, "00:10", Some(ahora.date_naive())));
    }

    #[test]
    fn una_hora_invalida_no_dispara_el_worker() {
        assert!(!debe_ejecutar(en_managua(2026, 9, 1, 0, 10), "24:00", None));
        assert!(!debe_ejecutar(
            en_managua(2026, 9, 1, 0, 10),
            "sin-hora",
            None
        ));
    }
}
