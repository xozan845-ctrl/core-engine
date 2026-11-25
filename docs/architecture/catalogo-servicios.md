<!-- GENERADO por scripts/tools/architecture-report.mjs. No editar a mano. -->
# Catálogo de servicios

Fuente de verdad: `packages/<servicio>/service.json` (R-MS-9). Este archivo se
regenera con `npm run architecture:report`; CI falla si está desactualizado.

| Servicio | Bounded context | Perfil | Puerto | Esquema | Capacidades | Entornos | Publica | Consume | Owner | Lifecycle |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [analytics-service](../../packages/analytics-service/README.md) | analytics | olap | 3016 | analytics | analitica_negocios | dev, stage, prod | 0 | 4 | core-platform | production |
| [api-gateway](../../packages/api-gateway/README.md) | gateway | edge | 8080 | — | — | dev, stage, prod | 0 | 0 | core-platform | production |
| [commissions-service](../../packages/commissions-service/README.md) | commissions | transactional | 3006 | commissions | comisiones, liquidaciones | dev, stage, prod | 2 | 4 | core-platform | production |
| [field-service](../../packages/field-service/README.md) | field | core | 3008 | field | operacion_campo | dev, stage, prod | 0 | 0 | core-platform | production |
| [finance-service](../../packages/finance-service/README.md) | finance | transactional | 3007 | finance | facturacion, facturacion_electronica | dev, stage, prod | 3 | 7 | core-platform | production |
| [identity-service](../../packages/identity-service/README.md) | identity | core | 3001 | identity | — | dev, stage, prod | 0 | 0 | core-platform | production |
| [inventory-service](../../packages/inventory-service/README.md) | inventory | transactional | 3013 | inventory | inventario_stock, inventario_lotes | dev, stage, prod | 4 | 3 | core-platform | production |
| [item-service](../../packages/item-service/README.md) | item | core | 3015 | item | catalogo_items | dev, stage, prod | 0 | 0 | core-platform | production |
| [logistics-service](../../packages/logistics-service/README.md) | logistics | transactional | 3005 | logistics | logistica_rutas, logistica_pod | dev, stage, prod | 2 | 2 | core-platform | production |
| [market-intelligence-service](../../packages/market-intelligence-service/README.md) | intelligence | olap | 3009 | intelligence | inteligencia_mercado | dev, stage, prod | 0 | 3 | core-platform | production |
| [notifications-service](../../packages/notifications-service/README.md) | notifications | consumer | 3012 | notifications | — | dev, stage, prod | 0 | 7 | core-platform | production |
| [orders-service](../../packages/orders-service/README.md) | orders | transactional | 3004 | orders | orders, fidelizacion, mesas, sala, suscripciones, contratos, cotizaciones, pagos | dev, stage, prod | 7 | 3 | core-platform | production |
| [pricing-service](../../packages/pricing-service/README.md) | pricing | core | 3014 | pricing | catalogo_items, cotizaciones | dev, stage, prod | 0 | 0 | core-platform | production |
| [scheduling-service](../../packages/scheduling-service/README.md) | scheduling | transactional | 3011 | scheduling | agenda_recursos, sala | dev, stage, prod | 3 | 1 | core-platform | production |
| [storage-service](../../packages/storage-service/README.md) | storage | transactional | 3010 | storage | — | dev, stage, prod | 2 | 0 | core-platform | production |
| [stores-service](../../packages/stores-service/README.md) | stores | consumer | 3003 | stores | tienda, resenas_verificadas | dev, stage, prod | 0 | 1 | core-platform | production |

## Perfiles

| Perfil | Significado |
|---|---|
| `edge` | Perímetro: JWT, políticas, proxy, rate limit. Sin base de datos. |
| `core` | Servicio síncrono con esquema propio, sin broker. |
| `transactional` | Publica eventos de dominio: exige outbox y RabbitMQ. |
| `consumer` | Consume eventos; no publica ni mantiene outbox propio. |
| `olap` | Analítica/proyecciones; no muta el dominio transaccional. |
