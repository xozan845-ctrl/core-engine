# ADR-0023 — Venta directa sin vendedor ni comisión de marketplace

- **Fecha:** 2026-11-18
- **Estado:** Aceptado (fase 1 implementada)
- **Relacionado:** [ADR-0001](./0001-plataforma-multi-vertical.md),
  [ADR-0013](./0013-orders-service-en-rust.md),
  [ADR-0019](./0019-fidelizacion-por-capacidades.md)

## Contexto

El flujo comercial de Core Engine nació como marketplace:

- `orders.orden_vista.vendedor_id` y `tienda_id` eran `NOT NULL`.
- `commissions-service` acreditaba una comisión por línea con la participación
  de la tienda.
- Una empresa única que vende directo (farmacia, clínica, distribuidora)
  podía operar, pero arrastraba vendedor y comisión.

El diagnóstico de agnosticismo
([`docs/audits/2026-11-18-agnosticismo.md`](../audits/2026-11-18-agnosticismo.md))
identificó este acoplamiento como el mayor bloqueador para vender a negocios
que no operan como marketplace.

## Decisión

1. **La venta directa se activa por configuración del pack**, no por código de
   cada rubro. Se añade el modelo operativo `venta_directa` con:
   ```json
   {
     "requiere_vendedor": false,
     "comision": false,
     "items": ["producto", "servicio", "alquiler"]
   }
   ```
   Un pack que incluya ese modelo desactiva el marketplace para su operación.
2. **El vendedor deja de ser obligatorio en la orden.** `orders.orden_vista`
   admite `vendedor_id NULL`; el contrato de la línea (`ItemOrden`) y la vista
   de orden lo exponen como nullable.
3. **La comisión es explícita por línea.** `ItemOrden` incorpora `comision`
   (default `true` para compatibilidad). Solo las líneas con vendedor y
   `comision = true` generan comisión; `commissions-service` las ignora cuando
   el flag es `false` o no hay vendedor.
4. **La tienda sigue siendo el canal de publicación.** Las ofertas continúan
   viviendo en `stores.ofertas` y la orden conserva `tienda_id`. La fase 1 no
   elimina la tienda; elimina el vendedor obligatorio y la comisión. Una tienda
   interna por empresa es una evolución posterior, no un requisito para vender
   directo.
5. **La comisión de la proyección sale del evento real.** Si el evento no trae
   `comision_cents`, la proyección usa `0`, nunca un 12 % implícito.

## Alternativas descartadas

- **Hacer nullable `tienda_id` y permitir ofertas sin tienda:** exige cambiar
  la unicidad y los joins de `stores.ofertas`, y deja sin storefront a los
  negocios que sí lo usan. Se pospone a una fase de canal directo.
- **Un tipo de orden separado (`venta_directa`):** duplica la máquina de
  estados y los consumidores sin aportar nada; el modo es una configuración de
  la operación, no un tipo de pedido.
- **Comisión 0 global por variable de entorno:** impide mezclar marketplace y
  venta directa en la misma plataforma. El flag por pack es más granular.

## Consecuencias

- **Positivas:** una empresa puede vender sin vendedor ni comisión; el
  marketplace sigue funcionando para los packs que no incluyen
  `venta_directa`; el cambio es aditivo y no rompe órdenes existentes.
- **Contrato:** `vendedor_id` pasa a nullable en `orders-service` (público e
  interno); `comision` se añade a la línea. Los consumidores deben tolerar
  `null`.
- **Datos:** migración aditiva `96_venta_directa.sql`; no se eliminan columnas
  ni filas.
- **Operación:** un pack de venta directa debe crearse o actualizarse con el
  modelo `venta_directa`; no se activa por defecto.
- **Pendiente:** tienda interna automática, ubicación de despacho y promociones
  aplicadas (fases posteriores del plan de agnosticismo).

## Criterios de aceptación

- Una orden creada con un pack `venta_directa` responde `vendedor_id: null` y
  `comision_cents: 0`.
- `commissions-service` no acredita ninguna línea con `comision = false` ni sin
  vendedor.
- Una orden de un pack marketplace conserva vendedor, comisión y autorización
  por vendedor.
- Los contratos OpenAPI público e interno declaran `vendedor_id` nullable y el
  flag `comision`.
- La suite de integración de orders cubre la orden directa y la de marketplace.
