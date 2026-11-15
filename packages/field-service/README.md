# Core Engine · field-service (Rust)

Reescritura en Rust de `field-service` con **paridad de contrato**. La decisión
y los invariantes están en
[`docs/adr/0008-field-service-en-rust.md`](../../docs/adr/0008-field-service-en-rust.md).

El crate vive en la raíz del paquete; la implementación TypeScript se retiró y
el servicio se construye con `cargo`.

## Comandos

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo run   # requiere DATABASE_URL (fail-fast)
```

Pruebas de integración (solo corren con una BD con el esquema `field`):

```bash
TEST_DATABASE_URL=postgres://user:pass@localhost:5432/core_engine_test cargo test
```

## Variables de entorno

| Variable | Default | Descripción |
|---|---|---|
| `PORT` | `3008` | Puerto HTTP interno |
| `SERVICIO` | `field-service` | Nombre en logs y métricas |
| `DATABASE_URL` | — | Obligatoria |
| `INTERNAL_API_KEY` | vacío | Clave de las APIs internas de identity |
| `IDENTITY_SERVICE_URL` | `http://identity-service:3001` | Capacidades y partes |
| `INTERNAL_REQUEST_TIMEOUT_MS` | `3000` | Timeout de llamadas internas |

`field-service` no usa RabbitMQ ni publica eventos: su superficie es HTTP +
PostgreSQL. `OUTBOX_TABLA` del compose se conserva por compatibilidad de
entorno, pero el servicio no lo lee.

## Alcance

- API pública `/api/v2/field/**` con envelope `{data, meta}` (R-C-1):
  personal, clientes, vehículos, rutas y paradas, pedidos, asistencia,
  incidencias, tracking GPS, visitas, cumplimiento y sync offline.
- Autorización: roles de campo (`super_admin`, `empresario`, `admin`,
  `logistica`, `coordinador`, `supervisor`, `operativo`), capacidad
  `operacion_campo` en todas las rutas y `logistica_rutas` en las rutas.
- Multi-tenancy por `field.*.tenant_id` + RLS con `request.jwt.claims`.
- DTOs con el mismo shape de error `VALIDACION`
  (`detalles[{campo, restricciones}]`).
- `/health` (`{servicio, uptime_seg}`), `/metrics` (Prometheus) y `/docs`
  (OpenAPI público versionado servido desde `docs/openapi/field-service/`).
- Apagado ordenado ante `SIGINT`/`SIGTERM`.
