# ADR-0024 — Ubicación de despacho por línea de orden

- **Fecha:** 2026-11-18
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0018](./0018-catalog-split-item-pricing-inventory.md),
  [ADR-0023](./0023-venta-directa-sin-marketplace.md)

## Contexto

`inventory-service` ya guarda stock por ubicación (`inventory.ubicaciones` +
`inventory.item_stock_ubicacion`), pero el checkout no elegía bodega. Al
descontar, se usaba la ubicación predeterminada y luego se repartía por mayor
stock; el cliente no podía decidir desde qué bodega despachar.

## Decisión

1. **La ubicación es opcional por línea.** `LineaEntrada` acepta
   `ubicacion_id`; `ItemOrden` lo persiste y lo propaga al evento `order.created`.
2. **El checkout valida la ubicación antes de crear la orden.** Si la línea
   trae `ubicacion_id`, orders consulta el stock de esa ubicación a inventory
   (`POST /internal/inventario/stock` con `ubicacion_id`). Si no alcanza, la
   orden se rechaza con `409 CONFLICTO`; si inventory no responde, `503`.
3. **Inventory reserva y descuenta en la ubicación elegida.** Sin
   `ubicacion_id` se conserva el comportamiento anterior (predeterminada y
   reparto). Con `ubicacion_id` el descuento es atómico sobre esa fila y falla
   si el stock físico no alcanza.
4. **La devolución reintegra a la ubicación elegida.** El evento conserva la
   ubicación y la compensación suma el stock allí.
5. **Una oferta no se parte entre ubicaciones en la misma orden.** Si la misma
   oferta aparece con dos `ubicacion_id`, el checkout responde
   `UBICACION_CONFLICTO`. El split shipment queda para una fase posterior.

## Consecuencias

- **Positivas:** una distribuidora puede despachar desde una bodega concreta;
  el stock por ubicación deja de ser informativo y participa del checkout.
- **Contrato:** `ubicacion_id` se añade como campo opcional en la línea de
  entrada y de salida (público e interno).
- **Lotes:** el consumo FEFO sigue siendo por empresa; los lotes no tienen
  ubicación propia en esta fase. La ubicación gobierna
  `item_stock_ubicacion` y la selección de series; la trazabilidad de lote se
  mantiene a nivel de empresa.
- **Sin ubicación:** el comportamiento legacy no cambia.

## Criterios de aceptación

- Una orden con `ubicacion_id` descuenta solo esa ubicación.
- Si la ubicación no existe, está inactiva o no alcanza, la reserva falla y no
  deja stock a medias.
- Una orden sin `ubicacion_id` mantiene el descuento por predeterminada.
- La devolución repone la misma ubicación.
- OpenAPI y README de orders/inventory documentan el campo.
