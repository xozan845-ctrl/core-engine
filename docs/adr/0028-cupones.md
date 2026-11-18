# ADR-0028 — Cupones de descuento por empresa

- **Fecha:** 2026-11-18
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0025](./0025-promociones-en-checkout.md)

## Contexto

Las promociones resolvían descuentos por item/cantidad, pero no existía un
código canjeable de orden (cupón) con vigencia, mínimo de compra y límite de
usos. Era una de las deudas de retail del diagnóstico de agnosticismo.

## Decisión

1. **Pricing es el dueño del cupón.** `pricing.cupones` guarda código, tipo
   (`porcentaje`/`monto_fijo`), valor, mínimo, tope, usos y vigencia.
   `pricing.cupon_usos` registra el canje por orden.
2. **Validación antes del checkout.** Orders envía
   `POST /internal/cupones/validar` con empresa, código y monto bruto. Pricing
   responde el descuento o un error de dominio (`CUPON_NO_ENCONTRADO`,
   `CUPON_VENCIDO`, `CUPON_MINIMO`, `CUPON_AGOTADO`).
3. **Canje idempotente.** Orders llama `canjear` con el `order_id`; pricing
   inserta `(cupon, orden)` una sola vez e incrementa `usos_actuales`. Si la
   orden no se persiste, orders llama `liberar` para compensar.
4. **El cupón es de orden y de una empresa.** Una orden multiempresa no admite
   cupón (`CUPON_UNA_EMPRESA`). Se aplica al total después de promociones.
5. **Trazabilidad.** `orders.orden_vista` guarda `descuento_cupon_cents` y
   `cupon_codigo`; la comisión se calcula sobre la base proporcional al cupón y
   finance descuenta el cupón de la base del comprobante.

## Consecuencias

- **Positivas:** cupones reales, canje único por orden, límite de usos y
  auditoría; el precio y la comisión quedan consistentes.
- **Contrato:** API pública `/api/v2/catalog/cupones`, endpoints internos de
  validar/canjear/liberar y campos de cupón en la orden.
- **Datos:** migraciones `99_cupones.sql` y `100_cupones_orders.sql`; RLS de
  cupones registrado en `99_rls.sql`.
- **Pendiente:** cupones acumulables con promociones, cupones por segmento de
  cliente y campañas automáticas.

## Criterios de aceptación

- Un cupón porcentual descuenta sobre el total y respeta el tope.
- Un cupón con mínimo o agotado devuelve el error de dominio.
- El canje repetido de la misma orden no incrementa usos.
- La orden guarda descuento y código; la comisión y el comprobante los reflejan.
