# ADRs de la plataforma multi-vertical

Este directorio contiene las decisiones de arquitectura que guían la
convergencia de Core Engine hacia una plataforma multi-vertical. El registro
histórico del repositorio permanece en [`../decisiones.md`](../decisiones.md);
este índice evita mezclar las decisiones nuevas con las ADR históricas.

## Índice

| ADR | Estado | Decisión |
|---|---|---|
| [0001](./0001-plataforma-multi-vertical.md) | Aceptado | Capacidades compartidas por tenant, tres pilotos iniciales y `scheduling-service` como nuevo bounded context |
| [0002](./0002-envelope-y-desacoplamiento.md) | Aceptado (fases 1-4 y 5-6 completas) | Envelope `{data,meta}` por servicio y desacoplamiento de `orders`/`scheduling`/`field` mediante APIs internas |
| [0003](./0003-notificaciones-correo.md) | Aceptado (fases 1-5 completas) | Proveedor de correo intercambiable, notificaciones del pedido y reset de contraseña |
| [0004](./0004-scheduling-service-en-rust.md) | Aceptado (implementado) | Reescribir `scheduling-service` en Rust con paridad de contrato, datos, eventos y métricas |
| [0005](./0005-autenticacion-gateway-sin-gate-de-borde.md) | Propuesto (fase 1) | Gateway sin Basic ni `/api-proxy`; API keys de `identity` intercambiadas por JWT corto |
| [0006](./0006-notifications-service-en-rust.md) | Aceptado (implementado) | Reescribir `notifications-service` en Rust con paridad de contrato, eventos, correo y métricas |
| [0007](./0007-commissions-service-en-rust.md) | Aceptado (implementado; pendiente smoke de staging) | Reescribir `commissions-service` en Rust con paridad de contrato, dinero, liquidaciones, eventos y métricas |
| [0008](./0008-field-service-en-rust.md) | Aceptado (implementado) | Reescribir `field-service` en Rust con paridad de contrato, datos y autorización |
| [0009](./0009-market-intelligence-service-en-rust.md) | Aceptado (implementado) | Reescribir `market-intelligence-service` en Rust con paridad de contrato, analítica y predicción |
| [0010](./0010-logistics-service-en-rust.md) | Aceptado (implementado) | Reescribir `logistics-service` en Rust con paridad de contrato, guías, flotas, eventos y métricas |
| [0011](./0011-stores-service-en-rust.md) | Aceptado (implementado) | Reescribir `stores-service` en Rust con paridad de contrato, tiendas, ofertas, tema y métricas |
| [0012](./0012-storage-service-en-rust.md) | Aceptado (implementado) | Reescribir `storage-service` en Rust con paridad de contrato, S3/MinIO, cuotas, media pública y eventos |
| [0013](./0013-orders-service-en-rust.md) | Aceptado (implementado; pendiente smoke de staging) | Reescribir `orders-service` en Rust con paridad de contrato, CQRS, Event Sourcing, carrito, contratos, suscripciones y salón |
| [0014](./0014-finance-service-en-rust.md) | Aceptado (implementado; pendiente smoke de staging) | Reescribir `finance-service` en Rust con paridad de contrato, contabilidad, tributación, facturación y métricas |
| [0015](./0015-api-gateway-en-rust.md) | Aceptado (implementado) | Reescribir `api-gateway` en Rust con paridad de contrato, JWT, políticas de borde, proxy, rate limit, readiness y métricas |
| [0016](./0016-catalog-service-en-rust.md) | Aceptado (implementado) | Reescribir `catalog-service` en Rust con paridad de contrato, items, precios, inventario, stock y eventos |
| [0017](./0017-identity-service-en-rust.md) | Aceptado (implementado) | Reescribir `identity-service` en Rust con paridad de contrato, JWT, sesiones, usuarios, empresas, personal, partes y métricas |
| [0018](./0018-catalog-split-item-pricing-inventory.md) | Aceptado (implementado; `catalog` retirado) | Separar `catalog-service` en `item-service`, `pricing-service` e `inventory-service` por bounded context |
| [0019](./0019-fidelizacion-por-capacidades.md) | Aceptado (implementado) | Fidelización como capacidad nueva, add-on seleccionable en el alta de empresa |
| [0020](./0020-contrato-v2-items.md) | Aceptado (fase 1 implementada) | Contrato interno v2 de items sin `precio` ni `stock`; precio y stock se resuelven en pricing/inventory |
| [0021](./0021-negocios-por-empresa.md) | Aceptado (fases 1-4, reportes, liquidaciones, selector, permisos, RLS, inventario, facturación, auditoría, administración, analítica, cohortes y reportes programados delegados a `analytics-service`) | `negocio` como unidad operativa completa con ciclo de vida, plantillas, transferencia de personal y analítica centralizada en `analytics-service` |
| [0022](./0022-analytics-service.md) | Aceptado | `analytics-service` como único dueño de la analítica de negocio, empresa y plataforma |
| [0023](./0023-venta-directa-sin-marketplace.md) | Aceptado (fase 1 implementada) | Venta directa configurable por pack: vendedor opcional y comisión explícita por línea |
| [0024](./0024-ubicacion-despacho-por-linea.md) | Aceptado (implementado) | Ubicación de despacho opcional por línea; inventory reserva y descuenta esa ubicación |
| [0025](./0025-promociones-en-checkout.md) | Aceptado (implementado) | Pricing resuelve la promoción vigente y orders aplica el descuento sobre el precio final |
| [0026](./0026-fiscalidad-por-tenant.md) | Aceptado (implementado; declaraciones globales) | Jurisdicción fiscal por tenant, moneda desde la jurisdicción e IVA por línea |
| [0027](./0027-pagos-proveedor-reembolsos.md) | Aceptado (implementado) | ACL de pasarela, cargo externo, reembolsos parciales, contracargo y conciliación |
| [0028](./0028-cupones.md) | Aceptado (implementado) | Cupones por empresa con vigencia, mínimo, tope, usos y canje idempotente |
| [0029](./0029-rma-devolucion-parcial.md) | Aceptado (implementado) | RMA por línea, restock idempotente y comisión proporcional |
| [0030](./0030-resenas-wishlist.md) | Aceptado (implementado) | Reseñas moderadas y wishlist por comprador |
| [0031](./0031-facturacion-electronica-pdf.md) | Aceptado (implementado) | Facturación electrónica/PDF opt-in por negocio con ACL de proveedor |
| [0032](./0032-expiracion-ordenes-impagas.md) | Aceptado (implementado) | TTL de órdenes impagas, `order.cancelled` y liberación idempotente de stock |
| [0033](./0033-idempotencia-http.md) | Aceptado (implementado) | `Idempotency-Key` en mutaciones críticas con replay de respuesta y detección de reutilización |
| [0034](./0034-ciclo-vida-tenant.md) | Aceptado (implementado) | Suspensión/cierre de empresa, bloqueo de login/API keys y revocación de sesiones |
| [0035](./0035-exportacion-borrado-retencion-tenant.md) | Aceptado (implementado en identity; orquestación cross-service pendiente) | Exportación, solicitud de eliminación, retención y purga de identidad |
| [0036](./0036-endurecimiento-autenticacion.md) | Aceptado (implementado) | Política de contraseñas, lockout, verificación de correo, MFA TOTP y sesiones |
| [0037](./0037-proveedores-pago-configurables.md) | Aceptado (implementado; adaptadores concretos pendientes) | Catálogo de proveedores de pago y selección por empresa/negocio |
| [0038](./0038-iva-por-linea-checkout.md) | Aceptado (implementado; opt-in por tenant) | IVA por línea, precios con/sin impuesto y snapshot fiscal en la orden |
| [0039](./0039-rma-reembolso-nota-credito.md) | Aceptado (implementado) | RMA con reembolso parcial automático y nota de crédito |
| [0040](./0040-pdf-comprobantes.md) | Aceptado (implementado) | PDF completo con emisor, cliente, líneas, IVA y almacenamiento |
| [0041](./0041-mvp-endurecimiento-ciclo-vida.md) | Aceptado (implementado) | MFA cifrado, reset admin, purga cross-service, retención de auditoría y nota de crédito electrónica |
| [0042](./0042-exportacion-asincrona-purga-total-pdf-storage.md) | Aceptado (implementado) | Exportación asíncrona firmada, purga en todos los servicios y PDF en object storage |
| [0043](./0043-cierre-p0-retail-flota.md) | Aceptado (implementado) | Analytics con devoluciones, pago manual explícito, notificaciones multicanal, POD, trigram y entrega de reportes |
| [0044](./0044-backup-restore-verificado.md) | Aceptado (implementado; PITR en backlog) | Backup lógico diario, retención, copia offsite manual y restore drill |
| [0045](./0045-webhooks-salientes.md) | Aceptado (implementado) | Webhooks salientes por tenant con HMAC, reintentos y API admin |
| [0046](./0046-suscripcion-plataforma-expansion.md) | Aceptado (implementado con cobro manual) | Plan USD 35/mes para operar más de un negocio |
| [0047](./0047-onboarding-idempotente.md) | Aceptado (implementado) | Onboarding autenticado, idempotente y con token/estado consolidado |
| [0048](./0048-pasarelas-tilopay-pagadito.md) | Aceptado (implementado; verificación sandbox pendiente) | Adaptadores Tilopay y Pagadito, webhooks como señal, conciliación programada y pago tardío |
| [0049](./0049-regiones-tributarias.md) | Aceptado (implementado; piloto Nicaragua) | Región tributaria seleccionable por pack y tenant, manifiesto fiscal y resolución fail-closed |

## Convención

- Los archivos usan `NNNN-kebab-case.md`.
- Cada ADR debe indicar fecha, estado, contexto, decisión, consecuencias y
  criterios de aceptación.
- Los cambios estructurales posteriores deben enlazar este ADR y registrar una
  nueva decisión o una enmienda explícita; no se modifica silenciosamente el
  alcance aceptado.
