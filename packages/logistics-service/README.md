# logistics-service · Rust

Reescritura en Rust de `logistics-service` con **paridad de contrato**. La
decisión y los invariantes están en
[`docs/adr/0008-logistics-service-en-rust.md`](../../docs/adr/0008-logistics-service-en-rust.md).

El crate vive en la raíz del paquete y el servicio se construye con `cargo`
(igual que `scheduling-service` y `notifications-service`).

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL y RABBITMQ_URL (fail-fast, R-MS-5)
```

Prueba de integración (solo corre con una BD con el esquema `logistics`):

```bash
TEST_DATABASE_URL=postgres://user:pass@localhost:5432/core_engine_test cargo test --test envios_pg
```

Variables de entorno: `PORT` (3005), `SERVICIO`, `DATABASE_URL`,
`RABBITMQ_URL`, `INTERNAL_API_KEY`, `IDENTITY_SERVICE_URL`,
`ORDERS_SERVICE_URL`, `PLATFORM_LOGISTICS_TENANT_ID`, `OUTBOX_TABLA`
(`logistics.outbox`).

## Alcance actual

- Configuración con `fail-fast` de variables obligatorias.
- `/health` (`{servicio, uptime_seg}`), `/metrics` (Prometheus) y `/docs`
  (OpenAPI público versionado servido desde `docs/openapi/logistics-service/`).
- Envelope `{data, meta}` en rutas públicas, errores `{codigo, mensaje,
  detalles?}`, `x-request-id` y logs JSON con contexto de correlación.
- Apagado ordenado ante `SIGINT`/`SIGTERM`.
- **Guías de despacho idempotentes** por `(order_id, empresa_id)`, con guía
  `BGH-...` y agrupación de líneas por empresa.
- **Flota derivada de capacidades** (`logistica_rutas`, `fulfillment_delivery`)
  consultadas a identity; fallo cerrado 503 si no responde.
- **Asignación híbrida** (`plataforma`/`empresa`) con bitácora en
  `logistics.asignaciones_envio` y protección de asignaciones fijas.
- **Transiciones**: envío por flota y orden vía endpoint interno de orders, con
  propagación de código/mensaje y aislamiento por tenant.
- **Consumidor** de `logistics.pagos` (`payment.procesado` y
  `order.status.updated`) con idempotencia por `event_id`; emite
  `shipment.started`, `order.completado` y `devolucion.solicitada`.
- **Outbox** (`OUTBOX_TABLA`) con poller de 1 s, lote de 50 y
  `FOR UPDATE SKIP LOCKED`.
- **RabbitMQ** (`lapin`): exchanges `core-engine.events`/`core-engine.dlq`,
  DLQ por cola, confirmaciones, reintentos `x-intentos` y republicación.
- **Imagen Docker** multi-stage (`rust:1.89-bookworm` → `debian:bookworm-slim`)
  con el mismo contrato de contenedor (puerto 3005).
- **Cobertura** medida con `cargo-llvm-cov` (gate 63/62 en CI).
