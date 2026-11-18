# ADR-0027 — Pagos con proveedor, reembolsos y conciliación

- **Fecha:** 2026-11-18
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0013](./0013-orders-service-en-rust.md),
  [ADR-0014](./0014-finance-service-en-rust.md)

## Contexto

Los pagos del MVP eran simulados: un webhook confirmaba/rechazaba y un admin
podía confirmar manualmente, pero no existía creación de cargo, conciliación,
reembolso ni chargeback. `orders.pagos` solo tenía un estado y una referencia
libre.

## Decisión

1. **ACL de pasarela (`ProveedorPagos`).** El dominio de orders no conoce al
   proveedor. La interfaz expone `crear_cargo`, `reembolsar` y `contracargar`.
   Hay dos adaptadores:
   - `ProveedorSimulado` (default, `PAGO_PROVEEDOR=simulado`).
   - `ProveedorHttp` (`PAGO_PROVEEDOR=http`) contra `PAGO_PROVEEDOR_URL` con
     `Authorization: Bearer PAGO_PROVEEDOR_KEY` y endpoints `/cargos`,
     `/reembolsos`, `/contracargos`.
2. **El cargo se crea al registrar el pago.** Los métodos con
   `config.cargo_externo=true` (o `simulado`) obtienen una referencia externa;
   los manuales (efectivo/transferencia) se registran como `proveedor=manual`.
3. **Reembolso total o parcial.** `monto_reembolsado_cents` acumula devoluciones
   y el estado pasa a `reembolsado_parcial` o `reembolsado`. La pasarela se
   invoca antes de persistir; si falla, no se marca el reembolso.
4. **Contracargo.** Se registra `contracargo` y un monto devuelto; no mueve la
   orden por sí solo (la decisión de negocio queda en el operador).
5. **Conciliación.** `conciliado_en` marca la revisión operativa.
6. **Auditoría.** Cada operación inserta en `orders.pagos_eventos`
   (`pago.reembolsado`, `pago.contracargo`, `pago.conciliado`).

## Consecuencias

- **Positivas:** el pago deja de ser un estado suelto; hay referencia externa,
  devoluciones parciales, chargeback y trazabilidad. Cambiar de pasarela no
  toca el dominio.
- **Configuración:** `PAGO_PROVEEDOR`, `PAGO_PROVEEDOR_URL`,
  `PAGO_PROVEEDOR_KEY` en `.env.*` y README.
- **Contrato:** la respuesta de pago expone `proveedor`,
  `monto_reembolsado_cents` y `conciliado_en`; se añaden tres rutas admin.
- **Pendiente:** conciliación automática contra el estado de la pasarela,
  webhooks de reembolso/contracargo y asiento contable automático del reembolso.

## Criterios de aceptación

- Un pago `simulado` nace con `proveedor=simulado` y una referencia externa.
- Un reembolso parcial acumula el monto y no supera el total.
- Un contracargo deja el pago en `contracargo` y no cambia la orden.
- `conciliar` marca `conciliado_en` y deja evento.
- La migración `98_pagos_reales.sql` es aditiva y los tests de integración
  cubren los tres flujos.
