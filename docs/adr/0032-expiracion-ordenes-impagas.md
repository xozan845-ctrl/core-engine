# ADR-0032 — Expiración de órdenes impagas y liberación de stock

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0013](./0013-orders-service-en-rust.md),
  [ADR-0018](./0018-catalog-split-item-pricing-inventory.md),
  [ADR-0027](./0027-pagos-proveedor-reembolsos.md)

## Contexto

`inventory-service` descuenta stock al consumir `order.created`, incluso cuando
el método de pago requiere confirmación manual. Si la orden queda en `creada`
porque el pago nunca se confirma ni se rechaza, la reserva no tiene vencimiento
y el stock queda retenido indefinidamente.

Además, solo el rechazo de pago y la transición a `devuelta` emitían
`devolucion.solicitada`. Una cancelación genérica (`PATCH /orders/{id}/estado`
con `cancelada`) no liberaba inventario.

## Decisión

1. **TTL de órdenes impagas.** `orders-service` ejecuta un barrido cada
   `ORDEN_EXPIRACION_INTERVALO_SEGUNDOS` (default 60 s) que cancela órdenes en
   `creada` con antigüedad mayor a `ORDEN_PAGO_TTL_MINUTOS` (default 30 min;
   `0` desactiva).
2. **Solo órdenes realmente impagas.** El barrido excluye cotizaciones y
   órdenes con un pago en estado `confirmado`, `reembolsado_parcial` o
   `reembolsado`. Así no compite con `stock.reservado` cuando el pago ya existe.
3. **Evento de liberación dedicado.** Toda transición a `cancelada` emite
   `order.cancelled` con `order_id`, `items` y `motivo`. Inventario lo consume
   y libera la reserva.
4. **Reversión financiera separada.** Solo si la orden estaba pagada
   (`pagada`, `en_preparacion`, `enviada` o `entregada`) se emite además
   `devolucion.solicitada` para que finanzas y comisiones reviertan la venta.
   Una cancelación impaga no genera asiento de devolución.
5. **Liberación idempotente.** `inventory.reintegrar_en_transaccion` devuelve
   `false` y no toca stock si no existe una reserva vigente. Esto cubre
   reintentos, eventos duplicados y el caso en que `order.cancelled` llega
   después de otro evento de liberación.
6. **Pagos pendientes rechazados.** Al cancelar, los pagos en estado
   `pendiente` de la orden pasan a `rechazado` para que un webhook o una
   confirmación tardía no intente mover una orden ya cancelada.
7. **Ubicación de origen.** `order.created`, `order.cancelled` y
   `devolucion.solicitada` conservan `item_id`/`ubicacion_id` para que el
   reintegro vuelva a la ubicación reservada.

## Consecuencias

- **Positivas:** el stock de una orden impaga se libera solo; la cancelación
  genérica y la expiración usan el mismo camino; no se duplican liberaciones ni
  asientos.
- **Contrato:** nuevo evento de outbox `order.cancelled`; inventario declara
  `order.cancelled` en su cola. No cambia ninguna ruta HTTP.
- **Configuración:** `ORDEN_PAGO_TTL_MINUTOS` y
  `ORDEN_EXPIRACION_INTERVALO_SEGUNDOS` en `.env.*` y README.
- **Datos:** no requiere migración; usa `orders.orden_vista`, `orders.pagos` e
  `inventory.reservas_ordenes`.
- **Pendiente:** TTL específico para cotizaciones; interacción entre una
  devolución parcial (RMA) y una cancelación total posterior; política de
  reintentos si RabbitMQ está caído más tiempo que el TTL.

## Criterios de aceptación

- Una orden con pago manual pendiente y antigüedad mayor al TTL pasa a
  `cancelada` y emite `order.cancelled`.
- Una orden con pago confirmado no expira.
- Inventario libera el stock exactamente una vez aunque reciba
  `order.cancelled` y `devolucion.solicitada` para la misma orden.
- Una cancelación impaga no emite `devolucion.solicitada`.
- Una cancelación pagada emite `order.cancelled` y `devolucion.solicitada`.
