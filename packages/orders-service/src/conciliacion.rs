//! Conciliación periódica de pagos con pasarela.
//!
//! El webhook de la pasarela solo es una señal: la decisión se toma siempre
//! consultando el estado real. Este barrido cubre los webhooks perdidos (red,
//! despliegue, reintentos agotados) consultando los pagos pendientes.

use crate::errors::ApiError;
use crate::logs;
use crate::pagos::{EstadoExterno, Pagos};
use crate::pagos_flujo::FlujoPagos;
use std::sync::Arc;

/// Actor de sistema para transiciones disparadas por conciliación.
const ACTOR_SISTEMA: &str = "00000000-0000-4000-8000-000000000000";

/// Tamaño de lote por ciclo para no saturar la pasarela.
pub const LOTE_MAXIMO: i64 = 200;

#[derive(Debug, Default, Clone, Copy)]
pub struct ResumenConciliacion {
    pub revisados: u64,
    pub confirmados: u64,
    pub rechazados: u64,
    pub errores: u64,
}

pub struct ConciliacionPagos {
    pagos: Arc<Pagos>,
    flujo: Arc<FlujoPagos>,
    minutos_gracia: u64,
    limite: i64,
}

impl ConciliacionPagos {
    pub fn nueva(
        pagos: Arc<Pagos>,
        flujo: Arc<FlujoPagos>,
        minutos_gracia: u64,
        limite: i64,
    ) -> Self {
        Self {
            pagos,
            flujo,
            minutos_gracia,
            limite: limite.max(1),
        }
    }

    /// Consulta un lote de pagos pendientes y aplica el estado real.
    ///
    /// Es idempotente: los pagos que ya cambiaron de estado dejan de aparecer
    /// en el siguiente barrido. Un fallo de red o de credenciales no tumba el
    /// lote; se registra y se reintenta en el siguiente ciclo.
    pub async fn ejecutar(&self) -> Result<ResumenConciliacion, ApiError> {
        let pendientes = self
            .pagos
            .pendientes_de_conciliacion(self.minutos_gracia, self.limite)
            .await?;
        let mut resumen = ResumenConciliacion {
            revisados: pendientes.len() as u64,
            ..ResumenConciliacion::default()
        };
        for mut pago in pendientes {
            let consulta = match self.pagos.consultar_pago(&pago, None).await {
                Ok(consulta) => consulta,
                Err(error) => {
                    resumen.errores += 1;
                    logs::warn(
                        "orders",
                        crate::campos! {
                            "msg": "conciliacion de pago fallida",
                            "pago_id": pago.id,
                            "proveedor": pago.proveedor,
                            "codigo": error.codigo,
                        },
                    );
                    continue;
                }
            };
            if let Some(liquidacion) = &consulta.liquidacion {
                if let Err(error) = self.pagos.aplicar_liquidacion(&mut pago, liquidacion).await {
                    resumen.errores += 1;
                    logs::warn(
                        "orders",
                        crate::campos! {
                            "msg": "no se pudo guardar la liquidacion de la pasarela",
                            "pago_id": pago.id,
                            "codigo": error.codigo,
                        },
                    );
                    continue;
                }
            }
            let resultado = match consulta.estado {
                EstadoExterno::Aprobado => {
                    // Primero la orden, después el pago: si la transición falla,
                    // el pago sigue pendiente y el próximo ciclo lo reintenta.
                    match self.flujo.confirmar(&pago, ACTOR_SISTEMA).await {
                        Ok(()) => match self.pagos.confirmar(&pago.id, ACTOR_SISTEMA).await {
                            Ok(_) => {
                                resumen.confirmados += 1;
                                Ok(())
                            }
                            Err(error) => Err(error),
                        },
                        Err(error) => Err(error),
                    }
                }
                EstadoExterno::Rechazado => {
                    match self
                        .flujo
                        .rechazar(&pago, "rechazado por conciliacion", ACTOR_SISTEMA)
                        .await
                    {
                        Ok(()) => {
                            match self
                                .pagos
                                .rechazar(&pago.id, "rechazado por conciliacion", ACTOR_SISTEMA)
                                .await
                            {
                                Ok(_) => {
                                    resumen.rechazados += 1;
                                    Ok(())
                                }
                                Err(error) => Err(error),
                            }
                        }
                        Err(error) => Err(error),
                    }
                }
                EstadoExterno::Pendiente => continue,
            };
            if let Err(error) = resultado {
                resumen.errores += 1;
                logs::warn(
                    "orders",
                    crate::campos! {
                        "msg": "no se pudo aplicar la conciliacion",
                        "pago_id": pago.id,
                        "codigo": error.codigo,
                    },
                );
            }
        }
        Ok(resumen)
    }
}
