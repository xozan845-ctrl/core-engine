# inventory-service · Rust

Primer corte de `catalog-service` según [ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md).
El servicio es dueño de stock, lotes FEFO, ubicaciones, movimientos, ajustes,
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
- PR 5: esquema `inventory`, `inventory.outbox`, RLS, gateway, compose, CI y
  retirada del consumidor viejo de `catalog-service`.
- `catalog.items.stock` sigue como proyección transitoria hasta `item-service`
  (PR 8); el resto del inventario ya es propiedad de `inventory-service`.

Las tablas propias viven en `inventory.*`. `catalog.items` y
`catalog.items.stock` siguen siendo el dato maestro transitorio del ítem hasta
la extracción de `item-service` (PR 8); no reactivar el consumidor de
`catalog.pedidos` en `catalog-service`.

## Comandos

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL y RABBITMQ_URL
```

Variables de entorno: `PORT` (3013), `SERVICIO`, `DATABASE_URL`,
`RABBITMQ_URL`, `INTERNAL_API_KEY`,
`IDENTITY_SERVICE_URL`, `ITEM_SERVICE_URL`, `OUTBOX_TABLA`
(`inventory.outbox`) e `INTERNAL_REQUEST_TIMEOUT_MS`.

## Contrato

El OpenAPI vive en
[`docs/openapi/inventory-service/`](../../docs/openapi/inventory-service/) y
entra en el contrato agregado del gateway.
Las rutas se conservan con el prefijo `/api/v2/admin/inventario`,
`/api/v2/admin/ubicaciones` y `/api/v2/catalog/inventario` para no romper el
gateway ni los clientes internos.
