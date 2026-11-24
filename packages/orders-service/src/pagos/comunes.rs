use super::prelude::*;
use super::Pago;

pub(super) fn fila_pago(fila: &sqlx::postgres::PgRow) -> Result<Pago, ApiError> {
    let confirmado_en: Option<DateTime<Utc>> = fila
        .try_get("confirmado_en")
        .map_err(crate::event_store::error_db)?;
    let conciliado_en: Option<DateTime<Utc>> = fila
        .try_get("conciliado_en")
        .map_err(crate::event_store::error_db)?;
    let creado_en: DateTime<Utc> = fila
        .try_get("creado_en")
        .map_err(crate::event_store::error_db)?;
    Ok(Pago {
        id: uuid_texto(fila, "id").map_err(crate::event_store::error_db)?,
        order_id: uuid_texto(fila, "order_id").map_err(crate::event_store::error_db)?,
        empresa_id: uuid_opcional(fila, "empresa_id").map_err(crate::event_store::error_db)?,
        metodo_codigo: fila
            .try_get("metodo_codigo")
            .map_err(crate::event_store::error_db)?,
        estado: fila
            .try_get("estado")
            .map_err(crate::event_store::error_db)?,
        monto_cents: i64::from(
            fila.try_get::<i32, _>("monto_cents")
                .map_err(crate::event_store::error_db)?,
        ),
        moneda: fila
            .try_get("moneda")
            .map_err(crate::event_store::error_db)?,
        referencia: fila
            .try_get("referencia")
            .map_err(crate::event_store::error_db)?,
        referencia_proveedor: fila
            .try_get("referencia_proveedor")
            .map_err(crate::event_store::error_db)?,
        proveedor: fila
            .try_get("proveedor")
            .map_err(crate::event_store::error_db)?,
        monto_reembolsado_cents: i64::from(
            fila.try_get::<i32, _>("monto_reembolsado_cents")
                .map_err(crate::event_store::error_db)?,
        ),
        motivo_rechazo: fila
            .try_get("motivo_rechazo")
            .map_err(crate::event_store::error_db)?,
        confirmado_por: uuid_opcional(fila, "confirmado_por")
            .map_err(crate::event_store::error_db)?,
        conciliado_en: conciliado_en.map(iso_ms),
        confirmado_en: confirmado_en.map(iso_ms),
        creado_en: iso_ms(creado_en),
    })
}

/// Referencia que espera la pasarela para consultar o modificar el pago.
pub(super) fn referencia_operativa(pago: &Pago) -> &str {
    if pago.referencia_proveedor.is_empty() {
        &pago.referencia
    } else {
        &pago.referencia_proveedor
    }
}
