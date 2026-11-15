# inventory-service · Rust

Primer corte de `catalog-service` según [ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md).
El servicio será dueño de stock, lotes FEFO, ubicaciones, movimientos, ajustes,
transferencias, reservas y configuración de inventario.

## Estado

Andamiaje (PR 2): configuración fail-fast, PostgreSQL con RLS, envelope,
errores, métricas, `/health`, `/metrics`, `/docs` y `/docs-json`. Las rutas de
dominio, el consumidor de la saga y RabbitMQ se incorporan en los PRs 3-5.

## Comandos

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL
```

Variables de entorno: `PORT` (3013), `SERVICIO`, `DATABASE_URL`,
`RABBITMQ_URL` (se exigirá en el PR 4), `INTERNAL_API_KEY`,
`IDENTITY_SERVICE_URL`, `ITEM_SERVICE_URL`, `OUTBOX_TABLA`
(`inventory.outbox`) e `INTERNAL_REQUEST_TIMEOUT_MS`.

## Contrato

El OpenAPI público vive en
[`docs/openapi/inventory-service/public.openapi.json`](../../docs/openapi/inventory-service/public.openapi.json).
Las rutas se conservan con el prefijo `/api/v2/admin/inventario`,
`/api/v2/admin/ubicaciones` y `/api/v2/catalog/inventario` para no romper el
gateway ni los clientes internos.
