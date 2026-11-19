# ADR-0039 — RMA con reembolso y nota de crédito parcial

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0027](./0027-pagos-proveedor-reembolsos.md),
  [ADR-0029](./0029-rma-devolucion-parcial.md),
  [ADR-0038](./0038-iva-por-linea-checkout.md)

## Contexto

El RMA por línea ya existía, pero al recibir una devolución solo restockeaba y
compensaba la comisión. No reembolsaba el pago ni emitía la nota de crédito, y
el importe devuelto no incluía el IVA snapshot de la orden.

## Decisión

1. **Importe por línea con IVA.** `orders.devolucion_lineas` guarda
   `base_cents`, `impuesto_cents`, `tasa_impuesto_bps` y `total_cents`. El total
   se prorratea desde `ItemOrden.total_linea_cents`.
2. **Reembolso automático al recibir.** `Devoluciones::recibir` intenta
   reembolsar el total del RMA contra pagos `confirmado`/`reembolsado_parcial`.
   Si no hay pago confirmado, queda `pendiente`; si el proveedor falla,
   `fallido`.
3. **Estado explícito.** `orders.devoluciones` guarda `reembolso_estado`,
   `reembolsado_cents`, `reembolso_error`, `nota_credito_id`,
   `nota_credito_estado` y `nota_credito_error`.
4. **Reintentos.** `POST /admin/devoluciones/{id}/reembolsar` y
   `POST /admin/devoluciones/{id}/nota-credito` permiten reintentar sin volver
   a recibir la mercancía.
5. **Nota de crédito.** Orders llama a
   `POST /internal/finance/notas-credito` con base, IVA y total del RMA. Finance
   emite una `NOTA_CREDITO` con serie propia y asiento inverso de IVA.
6. **Sin doble efecto.** El reembolso y la nota son idempotentes por estado; una
   devolución ya reembolsada/emitida no se repite.

## Consecuencias

- **Positivas:** el RMA cierra el ciclo físico, financiero y fiscal; el importe
  devuelto coincide con el IVA cobrado.
- **Contrato:** nuevos campos de RMA y dos rutas admin de reintento; endpoint
  interno de notas de crédito.
- **Datos:** migración `99_rma_reembolsos.sql`; serie `NC001` existente.
- **Límite:** el reembolso se hace contra el pago registrado en orders; si el
  cobro fue totalmente externo, queda `no_aplica`.
- **Límite:** la nota de crédito no envía todavía el PDF ni el XML electrónico
  al proveedor fiscal.

## Criterios de aceptación

- Recibir un RMA reembolsa el total parcial contra el pago confirmado.
- El pago queda `reembolsado_parcial` o `reembolsado` según el monto.
- Se emite una nota de crédito con base + IVA = total.
- Reintentar no duplica reembolso ni nota.
- Sin pago confirmado, el RMA queda `pendiente`/`no_aplica` sin bloquear la
  recepción de mercancía.
