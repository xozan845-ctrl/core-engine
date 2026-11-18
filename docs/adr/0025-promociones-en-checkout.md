# ADR-0025 — Promociones aplicadas en el precio real del checkout

- **Fecha:** 2026-11-18
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0018](./0018-catalog-split-item-pricing-inventory.md),
  [ADR-0023](./0023-venta-directa-sin-marketplace.md)

## Contexto

`pricing-service` ya tenía CRUD de promociones (`2x1`,
`descuento_cantidad`, `descuento_porcentaje`, `regalo`), pero ni orders ni
stores las aplicaban. La promoción era configuración visible sin efecto en el
precio real: el checkout cobraba el precio de la oferta sin descuento.

## Decisión

1. **Pricing sigue siendo el dueño de la regla comercial.** El endpoint
   interno `POST /internal/items/precios` resuelve, por línea, la promoción
   vigente y aplicable según `item_id`/`sku`, fecha y cantidad.
2. **Pricing no aplica el descuento monetario.** Devuelve un descriptor
   (`promocion`: tipo, cantidades, porcentaje) porque no conoce el margen de la
   oferta. **Orders** calcula el descuento sobre el precio final
   (`precio_unitario_cents × cantidad`) para respetar el margen.
3. **El descuento es por línea y explícito.** `ItemOrden` incorpora
   `descuento_cents` y `promocion_id`; el total de la orden resta el descuento y
   la comisión se calcula sobre el neto.
4. **Una promoción por línea.** Si hay varias vigentes, se usa la más reciente
   (orden determinista por `creado_en`, `id`). No se acumulan promociones en
   esta fase.
5. **Las reservas no aplican promociones** en esta fase: su precio se resuelve
   por tarifa de agenda.

## Consecuencias

- **Positivas:** la promoción cambia el precio real, queda en el evento y en la
  proyección, y es auditable por línea. El cálculo de comisión no se infla.
- **Contrato:** `promocion` en la respuesta interna de precios;
  `descuento_cents`/`promocion_id` en la línea de orden (público e interno).
- **Sin promoción:** el comportamiento no cambia (`descuento_cents = 0`).
- **Pendiente:** acumulación de promociones, cupones, promociones por método de
  pago y storefront.

## Criterios de aceptación

- Una orden con promoción porcentual descuenta el porcentaje sobre el precio
  final de la línea y el total refleja el neto.
- Un `2x1` con cantidad 4 descuenta 2 unidades.
- La comisión se calcula sobre el neto, no sobre el bruto.
- Sin promoción vigente, el descuento es 0.
- OpenAPI y README de pricing/orders documentan el descriptor y el descuento.
