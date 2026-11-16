# analytics-service · Rust

Único dueño de la analítica de negocio, empresa y plataforma.

## Alcance

- Consume `order.completado` y `comision.acreditada`.
- Proyecta `analytics.hechos_venta` y `analytics.comisiones`.
- Expone `/api/v2/analytics/**` con JSON/CSV y reportes programados.
- Consulta la orden a `orders-service` por la superficie interna.

`market-intelligence-service` sigue siendo el motor de mercado/geo; este
servicio es el dueño de la analítica de gestión.

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL y RABBITMQ_URL
```

Variables: `PORT` (3016), `SERVICIO`, `DATABASE_URL`, `RABBITMQ_URL`,
`INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`, `ORDERS_SERVICE_URL`.

Integración:

```bash
TEST_DATABASE_URL=postgres://user:pass@localhost:5432/analytics_test cargo test --test analitica_pg
```
