# ADR-0029 — RMA y devolución parcial por línea

- **Fecha:** 2026-11-18
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0013](./0013-orders-service-en-rust.md),
  [ADR-0018](./0018-catalog-split-item-pricing-inventory.md),
  [ADR-0028](./0028-cupones.md)

## Contexto

La devolución existente era total: rechazar un pago o transicionar la orden a
`devuelta` compensaba toda la comisión y reintegraba todo el stock. No había
RMA por línea ni cantidades parciales.

## Decisión

1. **Entidad RMA en orders.** `orders.devoluciones` + `orders.devolucion_lineas`
   guardan cantidades, motivo, estado (`solicitada`, `aprobada`, `recibida`,
   `rechazada`), total y `restock` por línea.
2. **Crear no mueve stock.** El comprador o un admin solicita la devolución;
   orders valida que la cantidad no supere lo comprado y lo ya devuelto.
3. **Recibir dispara el restock.** `POST /internal/inventario/devolucion`
   reintegra stock, series y lotes en una transacción idempotente por
   `devolucion_id` (`inventory.devoluciones_procesadas`).
4. **Comisión proporcional.** `devolucion.recibida` lleva `linea_clave` y
   cantidad; commissions compensa la parte proporcional del vendedor y del
   empresario. Una devolución total marca la comisión como `compensada`.
5. **La orden no cambia de estado por una devolución parcial.** El RMA es la
   fuente de verdad del tramo devuelto; la transición global a `devuelta` sigue
   existiendo para el caso total.

## Consecuencias

- **Positivas:** devoluciones parciales reales, restock idempotente y comisión
  proporcional; no se obliga a devolver toda la orden.
- **Contrato:** rutas de comprador/admin para devoluciones y endpoint interno de
  inventario; no cambia el contrato de la orden.
- **Datos:** migración `99_rma.sql`; `commissions.comisiones.cantidad` para
  calcular la proporción.
- **Pendiente:** notas de crédito automáticas, reembolso de pago vinculado al
  RMA y aprobación por flujo configurable.

## Criterios de aceptación

- Una devolución parcial no puede exceder la cantidad comprada.
- `recibir` restockea una sola vez aunque se reintente.
- La comisión se compensa en proporción a la cantidad devuelta.
- Una devolución total deja la comisión en `compensada`.
- OpenAPI y README documentan las rutas y el evento.
