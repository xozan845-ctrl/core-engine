//! Reversos de realización para devoluciones y RMA.
//!
//! El subledger `finance.ventas_lineas` conserva el reparto original por línea
//! y las cantidades ya revertidas. Así una devolución total y varias RMA
//! parciales nunca revierten dos veces la misma cantidad.

use crate::contabilidad::DetalleAsiento;
use crate::facturacion::OrdenResumen;
use crate::money;
use serde_json::Value;
use sqlx::{Postgres, Row, Transaction};
use std::collections::HashMap;

pub(crate) async fn tiene_subledger(
    tx: &mut Transaction<'_, Postgres>,
    order_id: &str,
) -> Result<bool, String> {
    let existe: Option<i32> =
        sqlx::query_scalar("SELECT 1 FROM finance.ventas_lineas WHERE order_id = $1::uuid LIMIT 1")
            .bind(order_id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(|e| e.to_string())?;
    Ok(existe.is_some())
}

/// Revierte las cantidades pendientes del subledger. `cantidades` limita el
/// reverso a las líneas y cantidades de una RMA; `None` revierte todo lo que
/// quede pendiente en la orden (cancelación/devolución total).
pub(crate) async fn reverso_realizacion(
    tx: &mut Transaction<'_, Postgres>,
    order_id: &str,
    cantidades: Option<&HashMap<String, i64>>,
) -> Result<Vec<DetalleAsiento>, String> {
    let filas = sqlx::query(
        "SELECT linea_clave, directa, cantidad, base_cents, impuesto_cents,
                comision_cents, vendedor_cents,
                revertido_cantidad, revertido_base_cents, revertido_impuesto_cents,
                revertido_comision_cents, revertido_vendedor_cents,
                revertido_empresario_cents
           FROM finance.ventas_lineas
          WHERE order_id = $1::uuid
          ORDER BY linea_clave
          FOR UPDATE",
    )
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;

    let mut total_comision = 0i64;
    let mut total_vendedor = 0i64;
    let mut total_empresario = 0i64;
    let mut total_directo = 0i64;
    let mut total_base = 0i64;
    for fila in filas {
        let linea_clave: String = fila.try_get("linea_clave").map_err(|e| e.to_string())?;
        let directa: bool = fila.try_get("directa").map_err(|e| e.to_string())?;
        let cantidad: i64 = i64::from(
            fila.try_get::<i32, _>("cantidad")
                .map_err(|e| e.to_string())?,
        );
        let base_cents: i64 = i64::from(
            fila.try_get::<i32, _>("base_cents")
                .map_err(|e| e.to_string())?,
        );
        let impuesto_cents: i64 = i64::from(
            fila.try_get::<i32, _>("impuesto_cents")
                .map_err(|e| e.to_string())?,
        );
        let comision_cents: i64 = i64::from(
            fila.try_get::<i32, _>("comision_cents")
                .map_err(|e| e.to_string())?,
        );
        let vendedor_cents: i64 = i64::from(
            fila.try_get::<i32, _>("vendedor_cents")
                .map_err(|e| e.to_string())?,
        );
        let revertido_cantidad: i64 = i64::from(
            fila.try_get::<i32, _>("revertido_cantidad")
                .map_err(|e| e.to_string())?,
        );
        let revertido_base: i64 = i64::from(
            fila.try_get::<i32, _>("revertido_base_cents")
                .map_err(|e| e.to_string())?,
        );
        let revertido_impuesto: i64 = i64::from(
            fila.try_get::<i32, _>("revertido_impuesto_cents")
                .map_err(|e| e.to_string())?,
        );
        let revertido_comision: i64 = i64::from(
            fila.try_get::<i32, _>("revertido_comision_cents")
                .map_err(|e| e.to_string())?,
        );
        let revertido_vendedor: i64 = i64::from(
            fila.try_get::<i32, _>("revertido_vendedor_cents")
                .map_err(|e| e.to_string())?,
        );
        let revertido_empresario: i64 = i64::from(
            fila.try_get::<i32, _>("revertido_empresario_cents")
                .map_err(|e| e.to_string())?,
        );

        let pendiente = (cantidad - revertido_cantidad).max(0);
        let solicitada = match cantidades {
            Some(mapa) => *mapa.get(&linea_clave).unwrap_or(&0),
            None => pendiente,
        };
        let delta_cantidad = solicitada.min(pendiente).max(0);
        if delta_cantidad == 0 {
            continue;
        }
        let nueva_cantidad = revertido_cantidad + delta_cantidad;
        let nueva_base = proporcional(base_cents, nueva_cantidad, cantidad);
        let nuevo_impuesto = proporcional(impuesto_cents, nueva_cantidad, cantidad);
        let delta_base = nueva_base - revertido_base;
        let delta_impuesto = nuevo_impuesto - revertido_impuesto;

        let (nuevo_revertido_comision, nuevo_revertido_vendedor, nuevo_revertido_empresario) =
            if directa {
                // La venta directa completa (base + impuesto) es acreedora del
                // empresario; el reverso devuelve ese pasivo a fondos por liquidar.
                let nuevo_total =
                    proporcional(base_cents + impuesto_cents, nueva_cantidad, cantidad);
                total_directo += nuevo_total - revertido_empresario;
                (0, 0, nuevo_total)
            } else {
                let nueva_comision = proporcional(comision_cents, nueva_cantidad, cantidad);
                let mut nueva_vendedor = proporcional(vendedor_cents, nueva_cantidad, cantidad);
                let mut nueva_empresario = nueva_base - nueva_comision - nueva_vendedor;
                if nueva_empresario < 0 {
                    // El redondeo nunca debe crear un pasivo negativo; el
                    // remanente se ajusta contra el vendedor.
                    nueva_vendedor = (nueva_base - nueva_comision).max(0);
                    nueva_empresario = 0;
                }
                total_comision += nueva_comision - revertido_comision;
                total_vendedor += nueva_vendedor - revertido_vendedor;
                total_empresario += nueva_empresario - revertido_empresario;
                total_base += delta_base;
                (nueva_comision, nueva_vendedor, nueva_empresario)
            };

        sqlx::query(
            "UPDATE finance.ventas_lineas
                SET revertido_cantidad = $2,
                    revertido_base_cents = $3,
                    revertido_impuesto_cents = $4,
                    revertido_comision_cents = $5,
                    revertido_vendedor_cents = $6,
                    revertido_empresario_cents = $7,
                    actualizado_en = NOW()
              WHERE order_id = $1::uuid AND linea_clave = $8",
        )
        .bind(order_id)
        .bind(nueva_cantidad as i32)
        .bind((revertido_base + delta_base) as i32)
        .bind((revertido_impuesto + delta_impuesto) as i32)
        .bind(nuevo_revertido_comision as i32)
        .bind(nuevo_revertido_vendedor as i32)
        .bind(nuevo_revertido_empresario as i32)
        .bind(&linea_clave)
        .execute(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    let mut detalles = Vec::new();
    if total_comision > 0 {
        detalles.push(DetalleAsiento {
            cuenta_codigo: "4.1".to_string(),
            debe_cents: total_comision,
            haber_cents: 0,
            concepto: Some("Comision revertida (RN-06)".to_string()),
            orden: None,
        });
    }
    if total_vendedor > 0 {
        detalles.push(DetalleAsiento {
            cuenta_codigo: "2.1.1".to_string(),
            debe_cents: total_vendedor,
            haber_cents: 0,
            concepto: Some("Liquidacion al vendedor revertida".to_string()),
            orden: None,
        });
    }
    if total_empresario > 0 {
        detalles.push(DetalleAsiento {
            cuenta_codigo: "2.1.6".to_string(),
            debe_cents: total_empresario,
            haber_cents: 0,
            concepto: Some("Liquidacion al empresario revertida".to_string()),
            orden: None,
        });
    }
    if total_directo > 0 {
        detalles.push(DetalleAsiento {
            cuenta_codigo: "2.1.6".to_string(),
            debe_cents: total_directo,
            haber_cents: 0,
            concepto: Some("Reverso de venta directa al empresario".to_string()),
            orden: None,
        });
    }
    let credito = total_base + total_directo;
    if credito > 0 {
        detalles.push(DetalleAsiento {
            cuenta_codigo: "2.1.2".to_string(),
            debe_cents: 0,
            haber_cents: credito,
            concepto: Some("Fondos por liquidar".to_string()),
            orden: None,
        });
    }
    Ok(detalles)
}

/// Compatibilidad con órdenes anteriores al subledger: revierte el último
/// asiento de realización o, si no existe, estima con la tasa configurada.
pub(crate) async fn reverso_legado(
    tx: &mut Transaction<'_, Postgres>,
    order_id: &str,
    orden: Option<OrdenResumen>,
    tasa: f64,
) -> Result<Vec<DetalleAsiento>, String> {
    let previo: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT id FROM finance.asientos
         WHERE referencia_tipo = 'order' AND referencia_id = $1
           AND concepto LIKE 'Realizacion de venta%' AND estado = 'REGISTRADO'
         ORDER BY creado_en DESC LIMIT 1",
    )
    .bind(order_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(|e| e.to_string())?;
    if let Some(previo) = previo {
        let filas = sqlx::query(
            "SELECT cuenta_codigo, debe_cents, haber_cents
             FROM finance.asiento_detalles WHERE asiento_id = $1",
        )
        .bind(previo)
        .fetch_all(&mut **tx)
        .await
        .map_err(|e| e.to_string())?;
        return Ok(filas
            .iter()
            .map(|fila| DetalleAsiento {
                cuenta_codigo: fila.try_get("cuenta_codigo").unwrap_or_default(),
                debe_cents: i64::from(fila.try_get::<i32, _>("haber_cents").unwrap_or(0)),
                haber_cents: i64::from(fila.try_get::<i32, _>("debe_cents").unwrap_or(0)),
                concepto: Some("Reverso por devolucion (RN-06)".to_string()),
                orden: None,
            })
            .filter(|detalle| detalle.debe_cents > 0 || detalle.haber_cents > 0)
            .collect());
    }
    let total = orden
        .map(|orden| orden.total_cents)
        .ok_or_else(|| "Orden no disponible para el asiento de devolucion.".to_string())?;
    let comision = money::comision(total, tasa).map_err(|error| error.mensaje)?;
    if comision <= 0 {
        return Ok(Vec::new());
    }
    Ok(vec![
        DetalleAsiento {
            cuenta_codigo: "4.1".to_string(),
            debe_cents: comision,
            haber_cents: 0,
            concepto: Some("Comision revertida".to_string()),
            orden: None,
        },
        DetalleAsiento {
            cuenta_codigo: "2.1.1".to_string(),
            debe_cents: 0,
            haber_cents: comision,
            concepto: Some("Compensacion en liquidacion".to_string()),
            orden: None,
        },
    ])
}

/// Redondeo proporcional entero (medio hacia arriba), acotado a `total_parte`.
pub(crate) fn proporcional(total: i64, parte: i64, total_parte: i64) -> i64 {
    if total <= 0 || parte <= 0 || total_parte <= 0 {
        return 0;
    }
    let parte = parte.min(total_parte);
    ((i128::from(total) * i128::from(parte) + i128::from(total_parte) / 2)
        / i128::from(total_parte)) as i64
}

/// Reverso de respaldo para RMA de órdenes sin subledger: los importes del
/// evento se tratan como venta directa, sin inventar una comisión.
pub(crate) fn reverso_desde_evento(items: &[Value]) -> Vec<DetalleAsiento> {
    let mut detalles = Vec::new();
    for item in items {
        let total = item
            .get("monto_cents")
            .and_then(Value::as_i64)
            .unwrap_or(0)
            .max(0);
        if total == 0 {
            continue;
        }
        detalles.push(DetalleAsiento {
            cuenta_codigo: "2.1.6".to_string(),
            debe_cents: total,
            haber_cents: 0,
            concepto: Some("Reverso RMA al empresario".to_string()),
            orden: None,
        });
        detalles.push(DetalleAsiento {
            cuenta_codigo: "2.1.2".to_string(),
            debe_cents: 0,
            haber_cents: total,
            concepto: Some("Fondos por liquidar".to_string()),
            orden: None,
        });
    }
    detalles
}
