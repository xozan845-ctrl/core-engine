<!-- GENERADO por scripts/tools/architecture-report.mjs. No editar a mano. -->
# Matriz de eventos

Topic: `core-engine.events`. Envelope: `tipo`, `event_id`, `data` y
metadatos de correlación (`x-request-id`). Fuente: `service.json` de cada
servicio; se regenera con `npm run architecture:report`.

| Evento | Publica | Consume |
| --- | --- | --- |
| `appointment.cancelled` | scheduling-service | — |
| `appointment.created` | scheduling-service | — |
| `appointment.reminder.due` | scheduling-service | — |
| `asiento.registrado` | finance-service | — |
| `comision.acreditada` | commissions-service | analytics-service, finance-service, market-intelligence-service |
| `comprobante.emitido` | finance-service | — |
| `declaracion.generada` | finance-service | — |
| `devolucion.recibida` | orders-service | analytics-service, commissions-service, finance-service |
| `devolucion.reembolsada` | orders-service | finance-service |
| `devolucion.solicitada` | orders-service | analytics-service, commissions-service, finance-service, inventory-service, notifications-service |
| `order.cancelled` | orders-service | inventory-service |
| `order.completado` | logistics-service | analytics-service, commissions-service, notifications-service |
| `order.created` | orders-service | finance-service, inventory-service, notifications-service, scheduling-service |
| `order.status.updated` | orders-service | logistics-service, notifications-service |
| `payment.procesado` | orders-service | commissions-service, finance-service, logistics-service, notifications-service |
| `shipment.started` | logistics-service | notifications-service |
| `stock.fallido` | inventory-service | notifications-service, orders-service |
| `stock.reintegrado` | inventory-service | orders-service |
| `stock.reservado` | inventory-service | orders-service |
| `stock.updated` | inventory-service | market-intelligence-service, stores-service |
| `storage.objeto.eliminado` | storage-service | — |
| `storage.objeto.subido` | storage-service | — |
| `venta.directa.acreditada` | commissions-service | finance-service |
| `venta.geolocalizada` | sistema externo | market-intelligence-service |

> Un evento sin publisher local puede ser externo (`venta.geolocalizada`) o
> deuda: la suite `services-contract` falla si un consumidor declara un evento
> que nadie publica y no está en `EXTERNAL_EVENTS`.
