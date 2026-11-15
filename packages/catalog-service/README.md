# catalog-service · Rust

Reescritura en Rust de `catalog-service` con **paridad de contrato**. La
decisión y los invariantes están en
[`docs/adr/0016-catalog-service-en-rust.md`](../../docs/adr/0016-catalog-service-en-rust.md).

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test                    # unitarias
TEST_DATABASE_URL=postgres://… cargo test   # + PostgreSQL y HTTP
cargo run                     # requiere DATABASE_URL y RABBITMQ_URL
```

Variables: `PORT` (3002), `SERVICIO`, `DATABASE_URL`, `RABBITMQ_URL`,
`INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`, `STORAGE_SERVICE_URL`,
`STORES_SERVICE_URL`, `OUTBOX_TABLA` (`catalog.outbox`),
`INTERNAL_REQUEST_TIMEOUT_MS`.

## Alcance

- Items unificados (producto/servicio/alquiler), atributos JSON Schema,
  variantes, imágenes, componentes, tarifas, series, listas de precio, precios
  y escalas.
- Inventario, lotes FEFO, ubicaciones, transferencias y movimientos: migrados
  a `inventory-service` (ADR-0018). El código legacy permanece sin rutas.
- Legacy `productos`/`servicios` con stock como proyección transitoria hasta
  la extracción de `item-service`.
- El consumidor `catalog.pedidos` se retiró; la saga vive en
  `inventory-service`.
- Superficie interna `/internal/items/**`.
- Envelope, errores normalizados, `/health`, `/metrics`, `/docs` y
  `/docs-json`.
- Imagen Docker multi-stage (puerto 3002) y cobertura con `cargo-llvm-cov`.
