# Core Engine · notifications-service (Rust)

Reescritura en Rust de `notifications-service` con **paridad de contrato**. La
decisión y los invariantes están en
[`docs/adr/0006-notifications-service-en-rust.md`](../../docs/adr/0006-notifications-service-en-rust.md).

El crate vive en la raíz del paquete; la implementación TypeScript se retiró y
el servicio se construye con `cargo`.

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL, RABBITMQ_URL y EMAIL_MODE válido (fail-fast)
```

Prueba de integración (solo corre con una BD con el esquema `notifications`):

```bash
TEST_DATABASE_URL=postgres://user:pass@localhost:5432/core_engine_test cargo test --test notificaciones_pg
```

## Variables de entorno

| Variable | Default | Descripción |
|---|---|---|
| `PORT` | `3012` | Puerto HTTP interno |
| `SERVICIO` | `notifications-service` | Nombre en logs y métricas |
| `DATABASE_URL` | — | Obligatoria (idempotencia) |
| `RABBITMQ_URL` | — | Obligatoria (consumidor) |
| `INTERNAL_API_KEY` | vacío | Clave de las APIs internas |
| `ORDERS_SERVICE_URL` | `http://orders-service:3004` | Detalle de la orden |
| `IDENTITY_SERVICE_URL` | `http://identity-service:3001` | Nombre y correo |
| `INTERNAL_REQUEST_TIMEOUT_MS` | `3000` | Timeout de llamadas internas |
| `EMAIL_MODE` | `log` | `log` o `smtp` |
| `EMAIL_FROM` | — | Remitente (obligatorio con `smtp`) |
| `SMTP_HOST` / `SMTP_PORT` | — / `587` | Relay SMTP |
| `SMTP_USER` / `SMTP_PASSWORD` | — | Credenciales opcionales, juntas |

## Alcance actual

- Configuración con `fail-fast` de `DATABASE_URL`, `RABBITMQ_URL` y correo.
- `/health` (`{servicio, uptime_seg}`), `/metrics` (Prometheus) y `/docs`
  (OpenAPI público versionado servido desde `docs/openapi/notifications-service/`).
  Sin envelope: el servicio no expone API pública de negocio.
- Errores de ruta `{codigo, mensaje}`, `x-request-id` y logs JSON con contexto.
- Apagado ordenado ante `SIGINT`/`SIGTERM`.
- **Consumidor** de `notifications.todos` con las mismas routing keys
  (`order.created`, `order.status.updated`, `payment.procesado`,
  `order.completado`, `shipment.started`, `devolucion.solicitada`,
  `stock.fallido`), prefetch 1 e idempotencia por `event_id` en
  `notifications.eventos_procesados`.
- **Resolución de datos** por las APIs internas de `orders` e `identity`, con
  UUID validado, timeout y 404 como “no disponible”; un 5xx/red se propaga para
  reintento.
- **Plantillas** del ciclo del pedido con el mismo texto, asuntos, montos,
  escape HTML y destinatarios que el TypeScript.
- **Correo** con `lettre`: `log` en dev/test (destinatario enmascarado, sin
  cuerpo) y SMTP con TLS en stage/prod.
- **RabbitMQ** (`lapin`): exchanges `core-engine.events`/`core-engine.dlq`,
  DLQ por cola, reintentos `x-intentos` (máx. 3) y republicación automática
  cada 10 s; supervisor de reconexión con backoff.
- **Imagen Docker** multi-stage (`rust:1.89-bookworm` → `debian:bookworm-slim`)
  con el mismo contrato de contenedor (puerto 3012).
- **Cobertura** con `cargo-llvm-cov` en el job `rust` de CI.
