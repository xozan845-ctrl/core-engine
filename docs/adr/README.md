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
| [0007](./0007-commissions-service-en-rust.md) | Aceptado (en curso) | Reescribir `commissions-service` en Rust con paridad de contrato, dinero, liquidaciones, eventos y métricas |
| [0008](./0008-field-service-en-rust.md) | Aceptado (implementado) | Reescribir `field-service` en Rust con paridad de contrato, datos y autorización |
| [0009](./0009-market-intelligence-service-en-rust.md) | Aceptado (implementado) | Reescribir `market-intelligence-service` en Rust con paridad de contrato, analítica y predicción |
| [0010](./0010-logistics-service-en-rust.md) | Aceptado (implementado) | Reescribir `logistics-service` en Rust con paridad de contrato, guías, flotas, eventos y métricas |
| [0011](./0011-stores-service-en-rust.md) | Aceptado (implementado) | Reescribir `stores-service` en Rust con paridad de contrato, tiendas, ofertas, tema y métricas |
| [0012](./0012-storage-service-en-rust.md) | Aceptado (en curso) | Reescribir `storage-service` en Rust con paridad de contrato, S3/MinIO, cuotas, media pública y eventos |
| [0013](./0013-orders-service-en-rust.md) | Aceptado (en curso) | Reescribir `orders-service` en Rust con paridad de contrato, CQRS, Event Sourcing, carrito, contratos, suscripciones y salón |
| [0014](./0014-finance-service-en-rust.md) | Aceptado (en curso) | Reescribir `finance-service` en Rust con paridad de contrato, contabilidad, tributación, facturación y métricas |
| [0015](./0015-api-gateway-en-rust.md) | Aceptado (implementado) | Reescribir `api-gateway` en Rust con paridad de contrato, JWT, políticas de borde, proxy, rate limit, readiness y métricas |
| [0016](./0016-catalog-service-en-rust.md) | Aceptado (implementado) | Reescribir `catalog-service` en Rust con paridad de contrato, items, precios, inventario, stock y eventos |
| [0017](./0017-identity-service-en-rust.md) | Aceptado (en curso; PR 1-3) | Reescribir `identity-service` en Rust con paridad de contrato, JWT, sesiones, usuarios, empresas, personal, partes y métricas |

## Convención

- Los archivos usan `NNNN-kebab-case.md`.
- Cada ADR debe indicar fecha, estado, contexto, decisión, consecuencias y
  criterios de aceptación.
- Los cambios estructurales posteriores deben enlazar este ADR y registrar una
  nueva decisión o una enmienda explícita; no se modifica silenciosamente el
  alcance aceptado.
