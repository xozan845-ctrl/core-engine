# ADR-0038 — IVA por línea en checkout

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado; opt-in por tenant)
- **Relacionado:** [ADR-0026](./0026-fiscalidad-por-tenant.md),
  [ADR-0028](./0028-cupones.md),
  [ADR-0049](./0049-regiones-tributarias.md)

## Contexto

El comprobante fiscal calculaba IVA a partir de las líneas que le pasaba
orders, pero orders no guardaba impuesto por línea ni lo mostraba en checkout.
El total de la orden y el total fiscal podían divergir, y el negocio no podía
elegir si sus precios incluían IVA.

## Decisión

1. **Configuración fiscal por tenant.** `finance.configuracion_fiscal` añade:
   - `impuesto_activo` (default `false` para no cambiar el comportamiento
     histórico).
   - `precios_con_impuesto` (default `true`).
2. **Tasa efectiva.** Orders consulta
   `GET /internal/finance/configuracion-fiscal` por `empresa_id`/`negocio_id` y
   recibe `tasa_iva`, `impuesto_activo`, `precios_con_impuesto` y moneda.
3. **Snapshot por línea.** `ItemOrden` guarda:
   - `impuesto_cents`
   - `tasa_impuesto_bps` (1500 = 15 %)
   - `total_linea_cents`
   El total de la orden y `orders.orden_vista.impuesto_cents` se derivan de ese
   snapshot.
4. **Cupón proporcional.** El descuento de cupón se reparte por línea antes de
   calcular IVA, para que el desglose sea coherente.
5. **Dos modos.**
   - `precios_con_impuesto=true`: el precio mostrado incluye IVA; la base es
     `total / (1 + tasa)` y el impuesto es la diferencia.
   - `false`: el precio es base; el impuesto se suma al total.
6. **Comprobante.** Finance usa el snapshot de la orden cuando existe
   (`total_linea_cents - impuesto_cents` como base) y deja de recalcular con la
   tasa actual. Órdenes antiguas sin snapshot conservan el cálculo legacy.
7. **Opt-in.** Sin `impuesto_activo=true` no cambia ningún total existente.

## Consecuencias

- **Positivas:** checkout y comprobante comparten el mismo desglose; el negocio
  decide si el precio incluye IVA; los tests y tenants actuales no cambian.
- **Contrato:** nuevos campos en ítems y orden; rutas internas de configuración
  fiscal y campos nuevos en la API pública de finanzas.
- **Datos:** migraciones `99_impuesto_config.sql` y `99_orders_impuestos.sql`;
  proyección `orders.orden_vista.impuesto_cents`.
- **Límite:** si finance no responde, orders no aplica impuesto (comportamiento
  actual). Para un régimen fiscal obligatorio conviene volverlo fail-closed.
- **Implementado (2026-11-23):** exenciones y múltiples tasas por línea vía
  `impuesto_codigo` contra el catálogo de la región, retención plana por región
  y desglose de tasa/exento/retención en el PDF. **Límite:** la retención es
  una tasa plana por región; regímenes con retenciones múltiples o por tipo de
  bien quedan como configuración adicional del catálogo.

## Criterios de aceptación

- Con impuesto inactivo, los totales no cambian.
- Con precios con IVA incluido, el total no cambia y se desglosa base/IVA.
- Con precios sin IVA, el total suma el impuesto.
- El comprobante usa el mismo total que la orden.
- El cupón reduce base e IVA de forma proporcional.

## Actualización (2026-11-23)

[ADR-0049](./0049-regiones-tributarias.md) conecta este desglose con la región
elegida por el tenant:

- La tasa, la moneda y el flag `impuesto_activo` se resuelven con la
  jurisdicción efectiva de finance, que ahora incluye
  `identity.empresas.region_tributaria` como fallback.
- `finance.configuracion_fiscal` sigue siendo el override explícito por empresa
  o negocio; la región es el valor por defecto.
- Si la serie del comprobante apunta a una región inexistente, finance responde
  `SIN_JURISDICCION` en lugar de aplicar `15 %`/`NIO` por su cuenta.
- Implementado: formato de moneda por región y series fiscales por
  empresa/región.
