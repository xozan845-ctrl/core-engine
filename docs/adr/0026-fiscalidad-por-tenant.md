# ADR-0026 — Jurisdicción fiscal por tenant y desglose de IVA por línea

- **Fecha:** 2026-11-18
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0014](./0014-finance-service-en-rust.md),
  [ADR-0025](./0025-promociones-en-checkout.md),
  [ADR-0049](./0049-regiones-tributarias.md)

## Contexto

`finance-service` tenía jurisdicciones configurables, pero:

- `jurisdiccion_activa()` tomaba la **primera activa ordenada por código de
  país**. Con dos países activos, el resultado dependía del alfabeto.
- `finance.comprobantes.moneda` estaba fijo en `C$`.
- El IVA del comprobante se calculaba sobre el total de la orden, no por línea,
  y no incorporaba descuentos por promoción.

## Decisión

1. **Configuración fiscal por tenant.** `finance.configuracion_fiscal` fija la
   jurisdicción por `empresa_id` y, opcionalmente, por `negocio_id`. La
   resolución es negocio → empresa → primera activa (compatibilidad legacy).
   API: `GET/PUT /api/v2/finanzas/configuracion-fiscal`.
2. **Moneda desde la jurisdicción.** El comprobante toma `moneda` de la
   jurisdicción de la serie; se elimina el `'C$'` literal.
3. **IVA por línea.** `facturacion.emitir` pide a orders el resumen con
   `items` (precio, cantidad, descuento), calcula base e IVA por línea y los
   suma. El IVA se calcula con la tasa de la jurisdicción de la serie.
4. **Compatibilidad.** Si la orden no trae `items` (órdenes legacy), se mantiene
   el cálculo sobre `total_cents`.
5. **Declaraciones.** Siguen siendo de plataforma en esta fase porque
   `finance.libro_mayor_mv` no tiene `empresa_id`; se documenta como límite y
   no se finge aislamiento por tenant.

## Consecuencias

- **Positivas:** dos empresas pueden facturar en jurisdicciones y monedas
  distintas; el IVA es auditable por línea y respeta descuentos.
- **Contrato:** nueva ruta pública de configuración fiscal; el resumen interno
  de orden acepta `items` (opcional).
- **Datos:** migración aditiva `97_fiscal_tenant.sql`; backfill de empresas
  existentes a `NI` si existe.
- **Pendiente:** declaraciones por tenant (requiere tenant en el libro mayor),
  facturación electrónica DGI y PDF.

## Criterios de aceptación

- `GET/PUT /finanzas/configuracion-fiscal` guarda y devuelve la jurisdicción
  del negocio/empresa.
- Un comprobante usa la moneda de su jurisdicción, no `C$` fijo.
- Con `items`, el IVA del comprobante es la suma del IVA por línea.
- Sin `items`, el comportamiento legacy se conserva.
- Tests de integración cubren configuración fiscal, moneda y comprobantes.

## Actualización (2026-11-23)

[ADR-0049](./0049-regiones-tributarias.md) completa la selección que este ADR
dejó como configuración posterior al alta:

- La jurisdicción ahora se elige **al aplicar el pack**, en `POST /empresas` y
  `POST /onboarding`, y se guarda en `identity.empresas.region_tributaria`.
- `finance.configuracion_fiscal` sigue existiendo como **override** explícito
  por empresa y por negocio. El orden es negocio → empresa → región del tenant
  → primera activa (compatibilidad legacy).
- El catálogo `finance.jurisdicciones` incorpora `moneda_iso`, `locale`,
  `zona_horaria`, `documento_fiscal` y `perfil`; `moneda` conserva el símbolo
  histórico que usan los comprobantes.
- Cambiar la región de la empresa realinea el override de empresa y respeta los
  overrides por negocio; no reescribe documentos ya emitidos.
- Sigue pendiente la declaración por tenant: el libro mayor no tiene
  `empresa_id`.
