# ADR-0037 — Proveedores de pago configurables por tenant

- **Fecha:** 2026-11-19
- **Estado:** Aceptado (implementado)
- **Relacionado:** [ADR-0027](./0027-pagos-proveedor-reembolsos.md)

## Contexto

El adaptador de pagos se elegía globalmente con `PAGO_PROVEEDOR`. Eso obliga a
un despliegue por pasarela y no permite que cada negocio active la suya. El
proyecto debe seguir siendo agnóstico: la pasarela es configuración, no una
bifurcación del dominio.

## Decisión

1. **Catálogo de proveedores.** `orders.proveedores_pago` guarda `codigo`,
   `nombre`, `tipo` (`simulado|http|manual`), `activo` y `config`.
2. **Selección por tenant.** `orders.configuracion_pagos` elige el proveedor por
   `empresa_id` y, opcionalmente, `negocio_id`. Si no hay configuración, se usa
   el proveedor global de `PAGO_PROVEEDOR` como fallback compatible.
3. **Credenciales fuera de la tabla.** `config.clave_env` y
   `config.webhook_secreto_env` son nombres de variables de entorno; el valor
   nunca se persiste en la base. `config.url` sí puede guardarse.
4. **ACL sin cambios.** El dominio sigue usando `ProveedorPagos`. Una fábrica
   resuelve el adaptador según el tipo del proveedor seleccionado.
5. **Proveedor manual.** `manual` no crea cargos ni reembolsa por API; sirve
   para efectivo/transferencia o conciliación externa.
6. **Webhook configurable.** El secreto del webhook se resuelve por pago →
   configuración del tenant → proveedor → variable de entorno, con fallback al
   `PAGO_WEBHOOK_SECRET` global.
7. **API administrativa.** `GET/POST /admin/proveedores-pago`,
   `PATCH /admin/proveedores-pago/{codigo}` y
   `GET/PUT /empresas/{id}/configuracion-pagos` (admin global o empresario del
   tenant).
8. **Extensibilidad.** Añadir Stripe/BAC/POS es añadir un `tipo` y su adaptador
   en la fábrica, sin tocar checkout, órdenes ni finanzas.

## Consecuencias

- **Positivas:** un mismo despliegue sirve a negocios con pasarelas distintas;
  activar una pasarela es configuración; las credenciales quedan en el entorno.
- **Contrato:** nuevas rutas administrativas y modelos de proveedor/config.
- **Datos:** migración `99_pagos_configurables.sql`; RLS para el rol de
  servicio; catálogo inicial `simulado`, `http` y `manual`.
- **Compatibilidad:** sin configuración por tenant se mantiene el proveedor
  global; los pagos existentes conservan su `proveedor`.
- **Pendiente:** adaptadores concretos (Stripe/BAC/POS), webhooks firmados
  específicos por proveedor y conciliación automática programada.

## Criterios de aceptación

- Un admin lista y crea proveedores.
- Un empresario configura el proveedor de su empresa y no puede configurar otra.
- Sin configuración por tenant se usa el proveedor global.
- Un pago con proveedor manual no crea cargos externos.
- Un pago con proveedor HTTP usa `config.url` y la clave de `clave_env`.
- El webhook acepta el secreto configurado por el proveedor/tenant.
