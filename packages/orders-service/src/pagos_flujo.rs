//! Transiciones de orden disparadas por el resultado de un pago.
//!
//! Vive fuera de la capa HTTP para que el webhook de la pasarela y el job de
//! conciliación avancen una orden con exactamente la misma lógica.

use crate::checkout::{Checkout, PAYMENT_PROCESADO};
use crate::errors::ApiError;
use crate::logs;
use crate::pagos::Pago;
use serde_json::json;
use std::sync::Arc;

pub struct FlujoPagos {
    checkout: Arc<Checkout>,
}

impl FlujoPagos {
    pub fn nuevo(checkout: Arc<Checkout>) -> Self {
        Self { checkout }
    }

    /// Marca la orden como `pagada` y emite el evento que dispara la venta
    /// (`PAYMENT_PROCESADO`). Idempotente si la orden ya avanzó.
    ///
    /// Si la orden ya está cerrada (por ejemplo, expiró mientras el comprador
    /// pagaba), el dinero se registró pero no se puede cumplir: se deja una
    /// incidencia para que un operador reembolse.
    pub async fn confirmar(&self, pago: &Pago, actor_id: &str) -> Result<(), ApiError> {
        let vista = self
            .checkout
            .vistas
            .encontrar(&self.checkout.db, &pago.order_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Orden", &pago.order_id))?;
        match vista.estado.as_str() {
            "pagada" => return Ok(()),
            "cancelada" | "devuelta" | "rechazada" => {
                self.checkout
                    .pagos
                    .registrar_incidencia(
                        pago,
                        "pago.tardio",
                        "pago aprobado con la orden ya cerrada; requiere reembolso",
                    )
                    .await?;
                logs::error(
                    "orders.pagos",
                    crate::campos! {
                        "msg": "Pago aprobado para una orden cerrada; revisar reembolso",
                        "pago_id": pago.id,
                        "order_id": pago.order_id,
                    },
                );
                return Ok(());
            }
            _ => {}
        }
        self.checkout
            .transicionar(
                &pago.order_id,
                "pagada",
                Some("pago_confirmado"),
                Some(actor_id),
                Some((
                    PAYMENT_PROCESADO,
                    json!({
                        "order_id": pago.order_id,
                        "monto_cents": pago.monto_cents,
                        "moneda": pago.moneda,
                        "estado": "procesado",
                        "metodo": pago.metodo_codigo,
                        "proveedor": pago.proveedor,
                        "proveedor_liquidacion": pago.proveedor_liquidacion,
                        "comision_pasarela_cents": pago.comision_pasarela_cents,
                        "impuesto_pasarela_cents": pago.impuesto_pasarela_cents,
                        "retencion_cents": pago.retencion_cents,
                        "monto_liquidado_cents": pago.monto_liquidado_cents,
                        "moneda_liquidacion": pago.moneda_liquidacion,
                        "detalle_pasarela": pago.detalle_pasarela,
                        "items": vista.items.iter().map(|item| json!({
                            "oferta_id": item.oferta_id,
                            "sku": item.sku,
                            "cantidad": item.cantidad,
                            "empresa_id": item.empresa_id,
                            "negocio_id": item.negocio_id,
                            "vendedor_id": item.vendedor_id,
                            "comision": item.comision,
                            "precio_unitario_cents": item.precio_unitario_cents,
                            "descuento_cents": item.descuento_cents,
                            "impuesto_cents": item.impuesto_cents,
                            "total_linea_cents": item.total_linea_cents,
                        })).collect::<Vec<_>>(),
                    }),
                )),
            )
            .await?;
        Ok(())
    }

    /// Cancela la orden rechazada. `transicionar` emite `order.cancelled` para
    /// liberar stock. Un rechazo ocurre antes de la venta, así que no hay
    /// asiento contable ni devolución que revertir.
    pub async fn rechazar(
        &self,
        pago: &Pago,
        motivo: &str,
        actor_id: &str,
    ) -> Result<(), ApiError> {
        let vista = self
            .checkout
            .vistas
            .encontrar(&self.checkout.db, &pago.order_id)
            .await?
            .ok_or_else(|| ApiError::no_encontrado("Orden", &pago.order_id))?;
        match vista.estado.as_str() {
            "creada" => {}
            "cancelada" => return Ok(()),
            _ => {
                self.checkout
                    .pagos
                    .registrar_incidencia(
                        pago,
                        "pago.rechazado_tardio",
                        "pago rechazado con la orden ya avanzada",
                    )
                    .await?;
                logs::warn(
                    "orders.pagos",
                    crate::campos! {
                        "msg": "Pago rechazado para una orden ya avanzada",
                        "pago_id": pago.id,
                        "order_id": pago.order_id,
                        "estado": vista.estado,
                    },
                );
                return Ok(());
            }
        }
        self.checkout
            .transicionar(
                &pago.order_id,
                "cancelada",
                Some(motivo),
                Some(actor_id),
                None,
            )
            .await?;
        Ok(())
    }
}
