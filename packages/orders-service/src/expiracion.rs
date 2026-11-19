//! Expiración de órdenes impagas.
//!
//! Una orden con pago pendiente mantiene stock reservado. Este barrido cancela
//! las órdenes que siguen en `creada` pasado el TTL y deja que `transicionar`
//! emita `order.cancelled` para que inventario libere la reserva.
//!
//! Solo considera órdenes sin un pago confirmado: una orden pagada que todavía
//! espera el evento `stock.reservado` no debe cancelarse.

use crate::checkout::Checkout;
use crate::db::Db;
use crate::errors::ApiError;
use crate::event_store::error_db;
use crate::logs;
use std::sync::Arc;
use uuid::Uuid;

const LOTE_MAXIMO: i64 = 500;

pub struct ExpiracionOrdenes {
    db: Arc<Db>,
    checkout: Arc<Checkout>,
    ttl_minutos: u64,
}

impl ExpiracionOrdenes {
    pub fn nueva(db: Arc<Db>, checkout: Arc<Checkout>, ttl_minutos: u64) -> Self {
        Self {
            db,
            checkout,
            ttl_minutos,
        }
    }

    /// Cancela un lote de órdenes impagas. Devuelve cuántas expiraron.
    ///
    /// Es idempotente: una orden que ya cambió de estado se ignora y el
    /// siguiente barrido no vuelve a verla.
    pub async fn expirar(&self) -> Result<u64, ApiError> {
        if self.ttl_minutos == 0 {
            return Ok(0);
        }
        let ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT v.id
               FROM orders.orden_vista v
              WHERE v.estado = 'creada'
                AND v.tipo_orden <> 'cotizacion'
                AND v.creado_en < NOW() - ($1::int * INTERVAL '1 minute')
                AND NOT EXISTS (
                      SELECT 1
                        FROM orders.pagos p
                       WHERE p.order_id = v.id
                         AND p.estado IN ('confirmado', 'reembolsado_parcial', 'reembolsado')
                )
              ORDER BY v.creado_en
              LIMIT $2",
        )
        .bind(self.ttl_minutos as i32)
        .bind(LOTE_MAXIMO)
        .fetch_all(self.db.pool())
        .await
        .map_err(error_db)?;

        let mut expiradas = 0u64;
        for id in ids {
            let order_id = id.to_string();
            match self
                .checkout
                .transicionar(
                    &order_id,
                    "cancelada",
                    Some("expirada_por_ttl"),
                    Some("sistema"),
                    None,
                )
                .await
            {
                Ok(_) => {
                    expiradas += 1;
                    logs::info(
                        "orders.expiracion",
                        crate::campos! {
                            "msg": "Orden impaga expirada",
                            "order_id": order_id,
                        },
                    );
                }
                // Otro proceso (pago, cancelación manual) ganó la carrera.
                Err(error) if error.codigo == "TRANSICION_INVALIDA" => {}
                Err(error) if error.codigo == "NO_ENCONTRADO" => {}
                Err(error) => return Err(error),
            }
        }
        Ok(expiradas)
    }
}
