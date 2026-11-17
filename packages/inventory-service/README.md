# inventory-service · Rust

Corte de inventario según [ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md).
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
  retirada del consumidor de pedidos heredado.
- PR 10: `inventory.item_stock_ubicacion` es la única fuente del stock; cada
  movimiento, consumo de lote y ajuste por ubicación publica `stock.updated`
  por `inventory.outbox` (RN-02).
- Fase 3b de ADR-0020: `item.items.stock` ya no existe. Los triggers de la
  proyección se retiraron y el alta de items usa
  `POST /internal/inventario/stock/inicial`.

El consumidor de pedidos vive en `inventory.pedidos` (no reactivar
`catalog.pedidos`).

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
