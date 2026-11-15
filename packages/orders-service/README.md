# orders-service (Rust)

Bounded context de pedidos: **CQRS + Event Sourcing** sobre `orders.eventos`, con
proyecciones mantenidas por trigger (`orden_vista`, `orden_timeline`,
`comisiones_vista`). Además del checkout expone carrito, entregas digitales,
contratos con depósito/garantía, suscripciones recurrentes y salón
(mesas/comandas).

Reescritura de paridad del servicio NestJS según
[ADR-0013](../../docs/adr/0013-orders-service-en-rust.md). El contrato HTTP no
cambia; el OpenAPI versionado vive en `docs/openapi/orders-service/`.

## Arquitectura

| Capa | Contenido |
|---|---|
| `http/` | Router axum, envelope `{data, meta}`, validación DTO, rutas públicas e internas |
| `checkout.rs` | Creación de órdenes, transiciones, replay y resumen de fulfillment |
| `event_store.rs` | Append-only con versión optimista y reintentos ante `23505` |
| `views.rs` | Proyección `orders.orden_vista` y timeline |
| `carrito.rs` | Carrito con expiración de 30 min (RN-05) |
| `consumer.rs` | Consumo de `stock.reservado` / `stock.fallido` idempotente |
| `contratos.rs` | Contratos, depósitos, garantías y movimientos |
| `suscripciones.rs` | Renovación horaria y órdenes recurrentes |
| `salon.rs` | Mesas y comandas por estación |
| `outbox.rs` / `rabbit.rs` | Outbox transaccional y bus de eventos |

## Endpoints principales

| Método | Ruta | Rol |
|---|---|---|
| POST | `/api/v2/orders` | comprador |
| POST | `/api/v2/orders/:id/cotizacion/{enviar,aceptar,rechazar}` | vendedor/comprador |
| GET | `/api/v2/orders`, `/api/v2/orders/:id`, `/:id/timeline` | autenticado |
| GET | `/api/v2/orders/{admin/todas,vendedor/mis-ventas,tienda/:id,empresa}` | según rol |
| GET/POST/PATCH/DELETE | `/api/v2/carrito[/items[/:ofertaId]]` | comprador |
| GET/POST/PATCH | `/api/v2/contratos[/:id[/finalizar|movimientos]]` | empresario/admin |
| GET/POST/PATCH | `/api/v2/suscripciones[/:id[/pausar|reanudar|cancelar|renovar|ejecuciones]]` | empresario/admin |
| GET/POST/PATCH | `/api/v2/mesas`, `/api/v2/comandas` | empresario/admin |
| GET | `/api/v2/pedidos/:id/entregas-digitales`, `/api/v2/entregas-digitales/:token` | comprador/admin |
| * | `/internal/orders/...` | clave interna |

## Variables de entorno

| Variable | Por defecto | Descripción |
|---|---|---|
| `DATABASE_URL` / `RABBITMQ_URL` | — (obligatorias) | PostgreSQL y broker |
| `PORT` | `3004` | Puerto HTTP |
| `INTERNAL_API_KEY` | — | Clave de las rutas internas |
| `OUTBOX_TABLA` | `orders.outbox` | Tabla del Outbox |
| `COMMISSION_RATE` | `0.12` | Tasa de comisión (RN-04) |
| `STORES_SERVICE_URL` | `http://stores-service:3003` | Ofertas y precios |
| `CATALOG_SERVICE_URL` | `http://catalog-service:3002` | Stock, precios y cotización |
| `IDENTITY_SERVICE_URL` | `http://identity-service:3001` | Capacidades |
| `SCHEDULING_SERVICE_URL` | `http://scheduling-service:3011` | Disponibilidad de reservas |
| `FINANCE_SERVICE_URL` | `http://finance-service:3007` | Crédito de órdenes |
| `INTERNAL_REQUEST_TIMEOUT_MS` | `3000` | Timeout de llamadas internas |

## Desarrollo

```bash
cd packages/orders-service
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test

# Integración PostgreSQL: provisiona su propio esquema `orders`.
TEST_DATABASE_URL=postgres://core_engine_test:test_only@127.0.0.1:5432/core_engine_test cargo test
```

La imagen multi-stage vive en `Dockerfile` y el compose de desarrollo apunta a
ella (`docker compose -f docker-compose.dev.yml up -d --build orders-service`).
