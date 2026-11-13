# market-intelligence-service · Rust

Reescritura en Rust de `market-intelligence-service` con **paridad de
contrato**. La decisión y los invariantes están en
[`docs/adr/0009-market-intelligence-service-en-rust.md`](../../docs/adr/0009-market-intelligence-service-en-rust.md).

El crate vive en la raíz del paquete y el servicio se construye con `cargo`.

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL y RABBITMQ_URL (fail-fast, R-MS-5)
```

Prueba de integración (requiere PostgreSQL con PostGIS y el esquema
`intelligence`; las pruebas lo provisionan desde `infra/db/init`):

```bash
TEST_DATABASE_URL=postgres://user:pass@localhost:5432/core_engine_test cargo test --test inteligencia_pg
```

Variables de entorno: `PORT` (3009), `SERVICIO`, `DATABASE_URL`,
`RABBITMQ_URL`, `INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`, `OUTBOX_TABLA`
(`intelligence.outbox`).

## Alcance actual

- Configuración con `fail-fast` de variables obligatorias.
- `/health`, `/metrics` (Prometheus) y `/docs` (OpenAPI público versionado).
- Envelope `{data, meta}`, errores `{codigo, mensaje, detalles?}`,
  `x-request-id` y logs JSON con contexto de correlación.
- Las **14 rutas** de métricas y predicción, con la misma validación de query
  (`desde`/`hasta` ISO 8601, `vendedor_id` UUID v4), defaults de `dias`,
  `limite` y `horas`, y el aislamiento de `/me/*` por `x-user-personal`.
- Capacidad `inteligencia_mercado` consultada a identity con fallo cerrado;
  admins globales pasan y el resto exige tenant.
- **Ingesta** de `venta.geolocalizada` (manual y por bus) con Data Contract
  JSON Schema draft-07, score de completitud High/Medium/Low y auditoría en
  `intelligence.invalid_events`.
- **Agregación atómica** de hechos, puntos de calor (PostGIS), rendimiento,
  cobertura por celda, métricas de producto (bucket de mercado, ADR-27) y
  tendencias; idempotencia por `event_id` o `md5(order_id)`.
- **Consumidor** de `intelligence.venta_geo`, `intelligence.order_completado`
  (`comision.acreditada`) e `intelligence.stock_updated`, con idempotencia por
  `event_id`.
- **Tareas**: features cada 5 min (media/desviación 7d, efectividad, churn,
  densidad 30d) y refresco concurrente de vistas materializadas cada 30 s.
- **Outbox** (`OUTBOX_TABLA`) con poller de 1 s, aunque el servicio no publica
  eventos de negocio.
- **Imagen Docker** multi-stage (`rust:1.89-bookworm` → `debian:bookworm-slim`),
  puerto 3009.
- **Cobertura** con `cargo-llvm-cov` (gate 63/60 en CI).
