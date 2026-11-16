# pricing-service · Rust

Motor de precios separado de `catalog-service` según [ADR-0018](../../docs/adr/0018-catalog-split-item-pricing-inventory.md).

## Estado

- Listas de precio, precios por lista, escalas por cantidad y precio base.
- Tarifas por vigencia y cotizaciones.
- Promociones (2x1, descuento por cantidad/porcentaje y regalo).
- Superficie interna para orders/stores: precios en lote, precio resuelto y cotización.
- Outbox `pricing.outbox` preparado para `price.changed`.

`item.items` es la fuente del item y de su precio base; este servicio resuelve listas, escalas, tarifas, cotizaciones y promociones, y solo actualiza el flag `tipo_precio` del item.

## Comandos

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL y RABBITMQ_URL
```

Variables: `PORT` (3014), `SERVICIO`, `DATABASE_URL`, `RABBITMQ_URL`,
`INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`, `ITEM_SERVICE_URL`,
`OUTBOX_TABLA` (`pricing.outbox`), `INTERNAL_REQUEST_TIMEOUT_MS`.
