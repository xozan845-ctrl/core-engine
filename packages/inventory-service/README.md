# inventory-service · Rust

Primer corte de `catalog-service` según [ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md).
El servicio será dueño de stock, lotes FEFO, ubicaciones, movimientos, ajustes,
transferencias, reservas y configuración de inventario.

## Estado

- PR 2: andamiaje con configuración fail-fast, PostgreSQL con RLS, envelope,
  errores, métricas, `/health`, `/metrics`, `/docs` y `/docs-json`.
- PR 3a: configuración de inventario, proveedores, niveles de stock, resumen y
  lotes FEFO con consumo atómico.
- PR 3b: ubicaciones, stock por ubicación, transferencias, movimientos,
  ajustes y superficie interna.
- PR 4: consumidor `inventory.pedidos` con reserva atómica, FEFO, series,
  componentes BOM, devolución, idempotencia, Outbox y DLQ.
- Pendiente: migración al esquema `inventory` y corte de gateway/clientes
  (PR 5).

Mientras dure la transición, el SQL apunta a `catalog.*`; el PR 5 mueve las
tablas al esquema propio y elimina esa dependencia.

> **No ejecutar `inventory-service` contra la misma base mientras
> `catalog-service` siga consumiendo `catalog.pedidos`.** Ambos descuentan
> stock y duplicarían la reserva. El corte de consumidores es parte del PR 5 y
> debe hacerse con el flag de gateway apagado hacia el servicio viejo.

## Comandos

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL
```

Variables de entorno: `PORT` (3013), `SERVICIO`, `DATABASE_URL`,
`RABBITMQ_URL`, `INTERNAL_API_KEY`,
`IDENTITY_SERVICE_URL`, `ITEM_SERVICE_URL`, `OUTBOX_TABLA`
(`inventory.outbox`) e `INTERNAL_REQUEST_TIMEOUT_MS`.

## Contrato

Mientras el servicio no está en `SERVICES`, el OpenAPI vive en
[`packages/inventory-service/openapi/`](./openapi/) para no colisionar con
`catalog-service` en el contrato agregado. En el PR 5 se mueve a
`docs/openapi/inventory-service/`.
Las rutas se conservan con el prefijo `/api/v2/admin/inventario`,
`/api/v2/admin/ubicaciones` y `/api/v2/catalog/inventario` para no romper el
gateway ni los clientes internos.
