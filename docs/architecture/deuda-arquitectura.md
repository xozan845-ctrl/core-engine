<!-- GENERADO por scripts/tools/architecture-report.mjs. No editar a mano. -->
# Deuda arquitectónica aceptada

El baseline `scripts/config/architecture-baseline.json` congela la deuda
existente (R-AF-2). **No puede crecer**: `npm run baseline:guard` rechaza una
entrada nueva y la suite `services-contract` falla si una entrada ya no
corresponde a un hallazgo real (debe retirarse en el mismo PR que la corrige).

Total: **29** entradas.

## cross-schema

Servicios que aún consultan o escriben esquemas ajenos (R-DB-1/R-MS-16).

| Servicio | Detalle |
|---|---|
| `analytics-service` | `identity` |
| `commissions-service` | `identity` |
| `commissions-service` | `orders` |
| `finance-service` | `identity` |
| `identity-service` | `finance` |
| `inventory-service` | `item` |
| `inventory-service` | `orders` |
| `item-service` | `identity` |
| `item-service` | `inventory` |
| `orders-service` | `identity` |
| `pricing-service` | `item` |
| `stores-service` | `identity` |

## env

Archivos que aún leen variables de entorno fuera de `config.rs`/`db.rs` (R-MS-5).

| Servicio | Detalle |
|---|---|
| `item-service` | `productos/helpers.rs` |
| `item-service` | `productos/mod.rs` |
| `orders-service` | `pagos/configuracion.rs` |
| `orders-service` | `pagos/webhooks.rs` |
| `orders-service` | `pasarelas/pagadito.rs` |
| `orders-service` | `pasarelas/tilopay.rs` |

## readiness

Servicios con base de datos que aún no exponen `/ready` (503 si una dependencia no está lista).

| Servicio | Detalle |
|---|---|
| `analytics-service` | `ready` |
| `commissions-service` | `ready` |
| `field-service` | `ready` |
| `item-service` | `ready` |
| `logistics-service` | `ready` |
| `market-intelligence-service` | `ready` |
| `notifications-service` | `ready` |
| `pricing-service` | `ready` |
| `scheduling-service` | `ready` |
| `storage-service` | `ready` |
| `stores-service` | `ready` |
