# ADR-0046 — Suscripción de plataforma para operar más de un negocio

- **Fecha:** 2026-11-20
- **Estado:** Aceptado (implementado con cobro manual)
- **Relacionado:** [ADR-0021](./0021-negocios-por-empresa.md),
  [ADR-0027](./0027-pagos-proveedor-reembolsos.md),
  [ADR-0045](./0045-webhooks-salientes.md)

## Contexto

El modelo de negocio del MVP define un único cobro de plataforma: **USD 35 al
mes** para el empresario que quiera operar **más de un negocio**. El primer
negocio (y la primera empresa) deben seguir siendo gratuitos para no romper el
alta. La plataforma ya distingue `empresa` (tenant fiscal) y `negocio` (unidad
operativa), y solo un empresario activo puede añadir negocios.

## Decisión

1. **Catálogo de planes en base de datos.**
   - `identity.planes_plataforma` contiene `empresario_plus` con
     `precio_cents = 3500`, `moneda = 'USD'`, `periodo_meses = 1`.
   - El precio no se hardcodea en Rust: el plan se lee de la tabla y se puede
     actualizar sin desplegar código.
2. **Suscripción por empresario.**
   - `identity.suscripciones_plataforma` guarda `usuario_id`, `empresa_id`
     opcional, plan, estado, origen, precio/periodo *snapshot*, fechas y
     referencia de pago.
   - Estados: `pendiente`, `activa`, `vencida`, `cancelada`. Una suscripción es
     vigente si `estado='activa'` y `fin_en` es nulo o futuro.
3. **Regla de expansión configurable.**
   - `PLATAFORMA_EMPRESAS_GRATIS` y `PLATAFORMA_NEGOCIOS_GRATIS` (ambos `1` por
     defecto) definen cuántas empresas/negocios se permiten sin plan.
   - Al superar el cupo se devuelve `402 PAYMENT_REQUIRED` con código
     `SUSCRIPCION_REQUERIDA` y el detalle del plan.
   - El administrador global queda exento.
   - Si el negocio decide cobrar desde el primer empresario, se configuran
     ambos cupos en `0`; no se cambia código.
4. **Cobro manual en el MVP.**
   - `POST /api/v2/plataforma/suscripciones` crea la solicitud `pendiente`.
   - Un admin confirma el pago con
     `POST /api/v2/admin/plataforma/suscripciones/{id}/confirmar`; se activa o
     renueva por `periodo_meses` desde `max(now, fin_en)`.
   - Cancelar corta la vigencia y solo bloquea nuevas expansiones; los negocios
     existentes siguen operando.
5. **Compatibilidad con tenants existentes.**
   - La migración crea una suscripción `legacy` activa con 30 días de gracia para
     cada empresario que ya tenía al menos una empresa.
   - Tras la gracia deben regularizar el cobro para añadir más negocios; no se
     les retiran los existentes.
6. **Pasarela real.**
   - `proveedor` y `referencia` quedan en el modelo para conectar Stripe/BAC/POS
     o una pasarela local sin cambiar el contrato de la regla de expansión.
   - La integración automática queda en el backlog de plataforma.

## Consecuencias

- **Positivas:** el cobro es explícito, auditable y no bloquea el alta; el
  primer negocio sigue siendo gratuito y la regla es configurable por entorno.
- **Operación:** el MVP depende de confirmación manual; si el volumen crece, se
  necesita una pasarela y conciliación automática.
- **Datos:** las suscripciones `legacy` no se cobran automáticamente; operación
  debe contactar a esos tenants o dejarlas expirar.
- **Límite:** no hay prorrateo, impuestos de plataforma ni factura de la
  suscripción; se documenta como evolución.

## Criterios de aceptación

- El primer negocio/empresa se crea sin plan.
- El segundo negocio/empresa devuelve `SUSCRIPCION_REQUERIDA` (402).
- Un admin confirma el pago y el siguiente negocio se crea.
- La suscripción muestra `vigente=true` hasta `fin_en`.
- Cancelar impide una nueva expansión, pero no desactiva los negocios existentes.
