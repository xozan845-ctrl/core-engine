# scheduling-service · Rust

Reescritura en Rust de `scheduling-service` con **paridad de contrato**. La
decisión y los invariantes están en
[`docs/adr/0004-scheduling-service-en-rust.md`](../../../../docs/adr/0004-scheduling-service-en-rust.md).

Durante la migración conviven el servicio TypeScript (`../src`) y este crate; el
PR de conmutación mueve el crate a la raíz del paquete y retira el código
anterior. **Este crate no se despliega todavía.**

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL y RABBITMQ_URL (fail-fast, R-MS-5)
```

Variables de entorno: `PORT` (3011), `SERVICIO`, `DATABASE_URL`,
`RABBITMQ_URL`, `INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`, `OUTBOX_TABLA`.

## Alcance de este andamiaje

- Configuración con `fail-fast` de variables obligatorias.
- `/health` (`{servicio, uptime_seg}`), `/metrics` (Prometheus) y `/docs`
  (OpenAPI público versionado servido desde `docs/openapi/scheduling-service/`).
- Envelope `{data, meta}` en rutas públicas, errores `{codigo, mensaje,
  detalles?}`, `x-request-id` y logs JSON con contexto de correlación.
- Apagado ordenado ante `SIGINT`/`SIGTERM`.

Pendiente en los PR 3-6: rutas de agenda, PostgreSQL/RLS, Outbox, RabbitMQ,
worker de recordatorios, consumidor de reservas e imagen Docker.
