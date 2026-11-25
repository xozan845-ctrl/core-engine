# Pendientes — cobro centralizado y dispersión diaria

- **Fecha:** 2026-11-25
- **Alcance:** [ADR-0053](./adr/0053-cobro-centralizado-dispersion-diaria.md)
- **Estado:** implementado en código; **no verificado con dinero real ni sandbox**

Este documento es la lista de cierre. No sustituye al runbook: lo que está aquí
no debe considerarse operativo hasta cumplir su criterio de aceptación.

## 1. Lo que ya está implementado

- `tarjeta` usa solo la cuenta global de la plataforma (`PAGO_PROVEEDOR`), no la
  pasarela configurada por un tenant.
- Se persisten fee de pasarela, IVA, retención y neto liquidado en
  `orders.pagos` y `commissions.pagos`.
- `commissions-service` cierra el día anterior, crea liquidaciones diarias y
  payouts separados para vendedor y empresa/negocio.
- Estados de payout: `pendiente → enviado → confirmado`, con `fallido` para
  reintento. Solo `confirmado` marca las liquidaciones como `pagada`.
- API admin para configurar proveedor, cuentas y payouts; consultas para
  empresario y vendedor.
- `commissions-service` entra en el compose de producción y en readiness.
- Retención configurable (`retencion_bps`) con default `0`.

## 2. Bloqueadores para cobrar/pagar de verdad

| # | Pendiente | Por qué bloquea | Criterio de aceptación |
|---|---|---|---|
| 1 | **Credenciales de cobro Tilopay/Pagadito** | `PAGO_PROVEEDOR=simulado` sigue en las plantillas. Con simulado el checkout rechaza `tarjeta`. | `PAGO_PROVEEDOR=tilopay|pagadito` con credenciales válidas; un cobro sandbox confirma la orden y llega el webhook. |
| 2 | **Contrato real de dispersión** | El adaptador `DispersionHttp` es configurable (`*_DISPERSION_URL`, `*_DISPERSION_PATH`, `*_DISPERSION_TOKEN`) pero el payload/autenticación exactos de la API de dispersión de Tilopay/Pagadito no están verificados. Sin URL falla cerrado. | Un payout sandbox se envía, devuelve referencia externa y se confirma con el proveedor. |
| 3 | **Consulta de estado del payout** | Hoy el payout se confirma por `POST .../confirmar` o por referencia devuelta al enviar. No hay job que consulte estado en el proveedor. | Un payout enviado se reconcilia automáticamente a `confirmado`/`fallido` sin intervención manual. |
| 4 | **Idempotencia y reintentos del proveedor** | Si el proveedor recibe la petición y la respuesta se pierde, el reintento podría duplicar el pago. | Clave de idempotencia enviada al proveedor y verificada en sandbox; reintento no duplica. |
| 5 | **Webhooks firmados de cobro** | Tilopay (`orderHash`) y Pagadito (firma asimétrica) no se validan localmente; el webhook solo se usa como señal y se consulta el estado. | Webhook con firma inválida rechazado; con firma válida no se puede replayar fuera de ventana. |
| 6 | **Fee/IVA/retención reales** | La extracción usa campos genéricos (`comision`, `fee`, `impuesto`, `retencion`, `monto_liquidado`, etc.). Hay que mapear los campos reales que devuelve cada pasarela y su unidad (unidades mayores vs centavos). | Un pago sandbox persiste exactamente el fee/IVA/neto del comprobante del proveedor. |
| 7 | **Conciliación contable fina** | Falta cruzar extracto de pasarela vs `orders.pagos` vs payouts vs banco, incluyendo diferencias de redondeo y tipo de cambio. | Reporte de conciliación diario sin diferencias sin explicar; diferencias clasificadas. |
| 8 | **Cuentas de destino y KYC** | `commissions.cuentas_pago` guarda un token/referencia, pero no hay validación de titularidad ni KYC del beneficiario. | Alta de cuenta validada con el proveedor; payout rechazado si el beneficiario no está verificado. |
| 9 | **Datos bancarios y secretos** | La referencia externa/token se guarda; los datos completos, si se usan, van en `detalle`. Falta definir cifrado/retención y quién puede verlos. | Datos sensibles cifrados o tokenizados; API no los expone; auditoría de acceso. |
| 10 | **Impuestos y retenciones por jurisdicción** | `retencion_bps` es un único porcentaje global. No distingue región, tipo de beneficiario ni base fiscal. | Reglas por región/tipo, retención declarada y remitida, comprobante por payout. |
| 11 | **Devoluciones y contracargos sobre payouts** | Las compensaciones reducen la siguiente liquidación, pero no hay clawback si el payout ya se envió. | Un reembolso posterior genera saldo negativo controlado o cobro al beneficiario, sin editar el histórico. |
| 12 | **Saldos negativos y mínimo diario** | Si el mínimo no se alcanza, la liquidación queda pendiente para el siguiente día. No hay política formal de saldo negativo. | Saldo negativo acumulado, límite y bloqueo/ajuste documentados. |
| 13 | **Aprobación y límites operativos** | El admin puede enviar/confirmar sin segundo aprobador ni límite por monto. | Límite diario/por payout y, si aplica, doble aprobación para montos altos. |
| 14 | **Producción y secretos** | `commissions-service` ya está en el compose, pero faltan secretos reales de dispersión y la verificación de que el worker corre con la hora local correcta. | `/ready` verde, worker ejecutando a la hora configurada y secretos en el gestor del entorno. |
| 15 | **Prueba E2E de cierre** | No se ha ejecutado el ciclo completo con sandbox. | Cobro → webhook → conciliación → cierre → payout → confirmación → reembolso, con evidencia en base. |

## 3. Checklist de cierre (sandbox)

1. Configurar credenciales de cobro y `PAGO_PROVEEDOR=tilopay|pagadito`.
2. Activar `tarjeta` y un proveedor real; crear una orden de prueba.
3. Verificar pago `confirmado`, orden `pagada` y fee/IVA/neto persistidos.
4. Perder el webhook a propósito y validar la conciliación por consulta.
5. Configurar `PUT /api/v2/admin/dispersion/configuracion` con el proveedor
   elegido y `retencion_bps` real.
6. Registrar cuentas de destino para vendedor y empresa/negocio.
7. Ejecutar `POST /api/v2/admin/dispersion/cerrar-dia` y verificar dos payouts.
8. Enviar cada payout y confirmar referencia externa.
9. Verificar que las liquidaciones quedaron `pagada` y el neto coincide con el
   extracto del proveedor.
10. Reembolsar total/parcial y comprobar compensación en el siguiente cierre.
11. Repetir el cierre dos días seguidos para validar idempotencia y reintentos.

## 4. Fuera de alcance de esta entrega

- No se probó con dinero real.
- No se implementó consulta automática de estado de payout en el proveedor.
- No se implementó KYC, doble aprobación ni límites por monto.
- No se implementó retención fiscal multi-región ni clawback de payouts ya
  enviados.
- No se modificó la selección de pasarela por tenant para métodos distintos de
  `tarjeta`.

## 5. Dónde está el código

- Cobro y persistencia: `packages/orders-service/src/pagos/`,
  `src/pasarelas/`, migración `infra/db/init/99z_pagos_liquidacion.sql`.
- Cierre y payouts: `packages/commissions-service/src/dispersion.rs`,
  `src/dispersion/`, `src/worker.rs`, migración
  `infra/db/init/99z_dispersion_diaria.sql`.
- Operación: `docs/runbook-lanzamiento-mvp.md` §1.7.
- Contrato: `docs/openapi/commissions-service/public.openapi.json`,
  `docs/openapi/orders-service/public.openapi.json`.
