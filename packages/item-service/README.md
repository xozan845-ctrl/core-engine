# item-service · Rust

Reescritura en Rust de `item-service` con **paridad de contrato**. La
decisión y los invariantes están en
[`docs/adr/0016-item-service-en-rust.md`](../../docs/adr/0016-item-service-en-rust.md).

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                    # unitarias
TEST_DATABASE_URL=postgres://… cargo test   # + PostgreSQL y HTTP
cargo run                     # requiere DATABASE_URL y RABBITMQ_URL
```

Variables: `PORT` (3015), `SERVICIO`, `DATABASE_URL`, `RABBITMQ_URL`,
`INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`, `STORAGE_SERVICE_URL`,
`STORES_SERVICE_URL`, `OUTBOX_TABLA` (`item.outbox`),
`INTERNAL_REQUEST_TIMEOUT_MS`.

## Alcance

- Items unificados (producto/servicio/alquiler), atributos JSON Schema,
  variantes, imágenes, componentes, series y combos.
- Precios, listas, escalas, tarifas, cotizaciones y promociones viven en
  `pricing-service`; el código duplicado se retiró de este crate (ADR-0018).
- Inventario, lotes, ubicaciones y movimientos viven en `inventory-service`.
  `item-service` no escribe tablas de inventario; solo valida la ubicación de
  una serie contra `inventory.ubicaciones`.
- `stock` y `precio_cents` de `item.items` se conservan como proyección de
  lectura del contrato de items. El stock se ajusta por
  `/api/v2/admin/inventario`; un `PATCH` con `stock` responde
  `STOCK_GESTIONADO_POR_INVENTARIO`.
- El consumidor de pedidos y la saga viven en `inventory-service`.
- Superficie interna v2 `/internal/v2/items/lote` y `/internal/v2/items/{id}`.
- Envelope, errores normalizados, `/health`, `/metrics`, `/docs` y
  `/docs-json`.
- Imagen Docker multi-stage (puerto 3015) y cobertura con `cargo-llvm-cov`.
