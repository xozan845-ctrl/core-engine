# scheduling-service · Rust

Reescritura en Rust de `scheduling-service` con **paridad de contrato**. La
decisión y los invariantes están en
[`docs/adr/0004-scheduling-service-en-rust.md`](../../docs/adr/0004-scheduling-service-en-rust.md).

El crate vive en la raíz del paquete desde la conmutación (ADR-0004); la
implementación TypeScript se retiró y el servicio se construye con `cargo`.

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL y RABBITMQ_URL (fail-fast, R-MS-5)
```

Prueba de integración (solo corre con una BD con el esquema `scheduling`):

```bash
TEST_DATABASE_URL=postgres://user:pass@localhost:5432/core_engine_test cargo test --test agenda_pg
```

Variables de entorno: `PORT` (3011), `SERVICIO`, `DATABASE_URL`,
`RABBITMQ_URL`, `INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`, `OUTBOX_TABLA`.

## Alcance actual

- Configuración con `fail-fast` de variables obligatorias.
- `/health` (`{servicio, uptime_seg}`), `/metrics` (Prometheus) y `/docs`
  (OpenAPI público versionado servido desde `docs/openapi/scheduling-service/`).
- Envelope `{data, meta}` en rutas públicas, errores `{codigo, mensaje,
  detalles?}`, `x-request-id` y logs JSON con contexto de correlación.
- Apagado ordenado ante `SIGINT`/`SIGTERM`.
- **Recursos, disponibilidad y bloqueos** (`/api/v2/scheduling/**`) con el
  mismo SQL, RLS por claims, locks `FOR UPDATE`, traducción de `23P01/23505` a
  `409 RECURSO_NO_DISPONIBLE` y verificación interna para `orders`.
- **Citas y recordatorios**: validación de parte en identity, exclusión GiST,
  recordatorios `24h`/`2h`, cancelación y procesamiento de vencidos.
- **Outbox** (`OUTBOX_TABLA`): eventos `appointment.created`,
  `appointment.cancelled` y `appointment.reminder.due` en la misma transacción.
- **Reservas** desde `order.created`: materialización idempotente por `order_id`.
- **RabbitMQ** (`lapin`): exchanges `core-engine.events`/`core-engine.dlq`, cola
  `scheduling.orders` con DLQ, confirmaciones, reintentos `x-intentos` y
  republicación automática de la DLQ.
- **Tareas de fondo**: publicador del Outbox cada 1 s y worker de recordatorios
  cada 60 s, con contexto de correlación de servicio.
- Capability guard contra la API interna de identity (fail-closed 503).

- **Imagen Docker** multi-stage (`rust:1.89-bookworm` → `debian:bookworm-slim`)
  con el mismo contrato de contenedor (puerto 3011, variables de entorno).
- **Cobertura** con `cargo-llvm-cov` y gate 71/65 (baseline 72,86/66,82), que
  sube +5 por release hasta el objetivo 94/77/88.

Pendiente en el PR 7: retirar el TypeScript y conmutar el servicio.
