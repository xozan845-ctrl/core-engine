# ADR-0053 — Cobro centralizado y dispersión diaria al negocio y al vendedor

- **Fecha:** 2026-11-25
- **Estado:** Aceptado (implementado; endpoints de dispersión sujetos a credenciales/sandbox)
- **Relacionado:** [ADR-0027](./0027-pagos-proveedor-reembolsos.md),
  [ADR-0037](./0037-proveedores-pago-configurables.md),
  [ADR-0048](./0048-pasarelas-tilopay-pagadito.md)
- **Cierre pendiente:** [pendientes-cobro-dispersion.md](../pendientes-cobro-dispersion.md)
- **Cierre de venta directa:** [2026-11-25-cierre-venta-directa.md](../audits/2026-11-25-cierre-venta-directa.md)

## Contexto

El modelo anterior permitía dos flujos incompatibles:

1. La plataforma configurara su propia pasarela (`PAGO_PROVEEDOR`) y cobrara
   en su cuenta.
2. Cada tenant seleccionara un proveedor en `orders.configuracion_pagos`, con
   lo que el dinero podía entrar directamente a la cuenta del comercio.

Además, `commissions-service` calculaba comisiones y creaba liquidaciones
quincenales, pero no existía un pago real al negocio ni al vendedor, y el
servicio no se desplegaba en producción. El requisito de negocio es único:

> El consumidor paga a la plataforma. La plataforma descuenta su comisión, paga
> su comisión al vendedor y entrega el resto al dueño de la empresa/negocio.

## Decisión

1. **La plataforma es el merchant of record.** El método `tarjeta` usa siempre
   el proveedor global de la plataforma (`PAGO_PROVEEDOR`), nunca la
   configuración de un tenant. Un pago histórico con tarjeta también se
   consulta/reembolsa contra ese proveedor.
2. **Fee de pasarela = costo de la plataforma.** La comisión que cobra
   Tilopay/Pagadito se resta de la comisión de la plataforma, no del neto del
   negocio ni del vendedor. El desglose se persiste en `orders.pagos`
   (`comision_pasarela_cents`, `impuesto_pasarela_cents`, `retencion_cents`,
   `monto_liquidado_cents`, `moneda_liquidacion`) y se replica en
   `commissions.pagos`.
3. **Impuestos con criterio de industria.**
   - El IVA de la comisión de plataforma es obligación de la plataforma y se
     calcula sobre su comisión, no sobre la venta completa.
   - Las retenciones sobre pagos al negocio/vendedor son configurables por
     jurisdicción (`retencion_bps` en `commissions.configuracion_dispersion`).
     Por defecto `0` hasta que fiscal defina la tasa; el monto retenido se
     guarda en el payout para declararlo y remitirlo.
4. **Cierre diario.** El worker de `commissions-service` cierra el día anterior
   a la hora local configurada (`00:10` por defecto), consolida las comisiones
   acreditadas en liquidaciones diarias y crea un payout por beneficiario:
   - `vendedor`: su comisión neta.
   - `negocio` o `empresa`: el remanente después de la comisión de plataforma.
5. **Dispersión por API.** La plataforma elige `tilopay` o `pagadito` como
   proveedor de dispersión. Cada payout se envía al adaptador HTTP con las
   credenciales del entorno (`*_DISPERSION_URL`, `*_DISPERSION_PATH`,
   `*_DISPERSION_TOKEN`). Sin URL configurada el adaptador falla cerrado; no se
   inventan endpoints ni se envía dinero a una ruta adivinada.
6. **Estado del payout.** `pendiente → enviado → confirmado`, con `fallido`
   para reintento y `cancelado` para anulación. Solo `confirmado` marca las
   liquidaciones incluidas como `pagada`.
7. **Cuentas de destino.** `commissions.cuentas_pago` guarda, por beneficiario y
   proveedor, la referencia externa/token. Los datos bancarios completos no se
   exponen en la API.
8. **Producción.** `commissions-service` pasa a desplegarse en producción; el
   gateway lo incluye en readiness y enruta `/admin/dispersion` y las consultas
   del portal.

## Consecuencias

- **Positivas:** un solo flujo de dinero; el negocio y el vendedor ven y cobran
  su parte; el fee y los impuestos quedan auditados; la plataforma puede
  cambiar de proveedor de dispersión sin tocar el dominio.
- **Datos:** migraciones `99z_pagos_liquidacion.sql` y
  `99z_dispersion_diaria.sql`; nuevas tablas `configuracion_dispersion`,
  `cuentas_pago`, `payouts` y `payout_items`.
- **Contrato:** endpoints admin de configuración/cuentas/payouts y consultas
  `empresarios/me/dispersion` y `vendedores/me/dispersion`.
- **Operación:** el adaptador de dispersión requiere que el contrato comercial
  y las credenciales de Tilopay/Pagadito estén verificados en sandbox. Hasta
  entonces los payouts quedan `pendiente`/`fallido` y no se marca nada como
  pagado.
- **Compatibilidad:** los métodos distintos de tarjeta conservan la selección
  por tenant; las liquidaciones quincenales históricas siguen disponibles.

## Criterios de aceptación

- Un pago con `tarjeta` ignora la configuración de pasarela del tenant y usa la
  cuenta global.
- La respuesta/consulta de la pasarela persiste fee, impuesto, retención y neto.
- El cierre diario crea payouts separados para vendedor y empresa/negocio.
- La retención configurada se descuenta del payout y el neto se envía.
- Un payout solo pasa a `confirmado` con referencia del proveedor o confirmación
  manual de un admin, y entonces sus liquidaciones quedan `pagada`.
- Sin credenciales de dispersión el sistema no marca pagos como realizados.
