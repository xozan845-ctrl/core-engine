# ADR-0006: Reescritura de `notifications-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-12
- **Estado**: Aceptado — implementación completa; pendiente el smoke de `staging` antes del merge a `develop`
- **Ámbito**: `notifications-service` (consumidor RabbitMQ, PostgreSQL, correo, métricas)
- **Referencias**: [ADR-0003](./0003-notificaciones-correo.md), [ADR-0004](./0004-scheduling-service-en-rust.md)

## Contexto

`notifications-service` es un servicio NestJS pequeño (~300 líneas) que:

- Consume la cola `notifications.todos` con los eventos del ciclo del pedido
  (`order.created`, `order.status.updated`, `payment.procesado`,
  `order.completado`, `shipment.started`, `devolucion.solicitada` y
  `stock.fallido`); no publica eventos.
- Resuelve comprador, vendedor y detalle de la orden por las APIs internas de
  `identity` y `orders`; no lee esquemas ajenos.
- Es idempotente por `event_id` en `notifications.eventos_procesados`; los
  reintentos y la DLQ son los de `RabbitService`.
- Construye plantillas de correo y envía con `@core/shared/email`: `log` en
  dev/test y SMTP en stage/prod.
- No expone HTTP público de negocio: solo `/health`, `/metrics` y `/docs`.

Se decide reescribirlo en Rust **sin cambiar contratos, modelos ni lógica**,
siguiendo el patrón ya validado en `scheduling-service` (ADR-0004). El servicio
conserva nombre, puerto, variables de entorno, esquema de base de datos,
topología de eventos, textos de correo y respuestas HTTP.

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie HTTP | solo `/health`, `/metrics` y `/docs`; sin envelope `{data, meta}`; 404 `{codigo, mensaje}`; `x-request-id` |
| Health | `{servicio, uptime_seg}` |
| Observabilidad | `/metrics` Prometheus con `core_engine_http_*` y etiquetas `{servicio, metodo, ruta, estado}`; logs JSON con `request_id`, `servicio` y nivel |
| Consumo | cola `notifications.todos`, las 7 routing keys y prefetch 1 |
| Idempotencia | `notifications.eventos_procesados(event_id, tipo)`; si el envío falla no se marca; si ya existe se descarta |
| Datos externos | `orders` para la orden y `identity` para los usuarios; UUID validado; 404 = “no disponible”; red/5xx se propaga para reintento |
| Plantillas | mismos asuntos, textos, montos (`C$`/`US$`), escape HTML y destinatarios que el TypeScript |
| Correo | `EMAIL_MODE=log|smtp`; en `log` se enmascara el destinatario y no se registra cuerpo; SMTP con TLS |
| RabbitMQ | exchanges `core-engine.events`/`core-engine.dlq`; DLQ por cola; `x-intentos` máximo 3; republicación cada 10 s; supervisor con backoff |
| Runtime | puerto 3012 (`PORT`), fail-fast si faltan `DATABASE_URL`, `RABBITMQ_URL` o la configuración SMTP; cierre ordenado ante `SIGINT`/`SIGTERM` |
| Imagen | mismo contrato de contenedor (puerto 3012) y usuario sin privilegios |

### 2. Implementación destino

- Crate en la raíz de `packages/notifications-service/`, con binario
  `notifications-service`; se retira el TypeScript en la conmutación.
- `axum` + `tokio` para HTTP, `sqlx` para PostgreSQL, `lapin` para RabbitMQ,
  `reqwest` para las APIs internas y `lettre` para SMTP.
- Capas: `http/` (router, salud, métricas, correlación), `clientes/`, `email/`,
  `plantillas.rs`, `notificaciones.rs`, `consumidor.rs`, `rabbit.rs`, `db.rs`,
  `config.rs`.
- Inyección explícita por estado compartido (`AppState` con `Arc`), equivalente
  verificable de la DI de NestJS (enmienda de R-MS-3).

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y
   `cargo test` verdes.
2. Prueba de integración PostgreSQL (`tests/notificaciones_pg.rs`): idempotencia
   por `event_id` y no-marcado cuando el envío falla.
3. Los OpenAPI versionados no cambian; el binario sirve los mismos documentos.
4. Smoke de `staging` con RabbitMQ real (evento → correo en modo `log` o SMTP
   local) y `npm run demo`.
5. Cobertura Rust en el job `rust` de CI (`cargo-llvm-cov`).

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | ADR-0006 y actualización del índice | Decisión registrada |
| 2 | Crate Rust: configuración, HTTP, métricas, correo y plantillas | `cargo test` verde con paridad de textos |
| 3 | Consumidor, idempotencia, RabbitMQ y prueba PostgreSQL | Integración verde |
| 4 | Imagen Docker y job `rust` en matriz | Build y cobertura verdes |
| 5 | Conmutación: retirar TypeScript, scripts raíz, compose y docs | `npm run demo` y smoke verdes |
| 6 | Merge a `develop` | Smoke de `staging` y checks verdes |

## Consecuencias

### Positivas

- Un servicio menos en el runtime NestJS y menos consumo en reposo.
- Tipado estricto en clientes, plantillas y correo.
- La migración reutiliza el patrón y los gates de ADR-0004.

### Costes y riesgos

- `lettre` es una dependencia nueva en el inventario Rust.
- La paridad del correo es sensible a detalles (espacios, acentos, `#corto`,
  escape HTML); queda cubierta por pruebas de plantillas.
- Durante la conmutación conviven la imagen TS y la Rust; solo una se despliega.

## Criterios de aceptación

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y la
  integración PostgreSQL en verde.
- Los eventos se procesan una sola vez y el fallo de correo no marca el evento.
- OpenAPI, salud y métricas conservan el contrato.
- `smoke-verticals` / `npm run demo` y el smoke de `staging` verdes antes del
  merge a `develop`.

## Estado de implementación (2026-11-12)

- Crate completo en `packages/notifications-service/` con 56 pruebas unitarias
  y 2 de integración PostgreSQL.
- TypeScript retirado; `scripts/config/project.mjs` lo incluye en
  `RUST_SERVICES`, `package.json` ya no lo compila ni lo prueba y su umbral
  Vitest se retiró del baseline.
- Job `rust` de CI convertido en matriz (`scheduling-service` y
  `notifications-service`) con gates de cobertura 71/65 y 74/74.
- Cobertura medida con PostgreSQL efímero: 75,36 % líneas / 75,38 % funciones.
- Imagen Rust verificada con PostgreSQL y RabbitMQ reales: `/health`,
  `/metrics`, cola `notifications.todos` y su DLQ declaradas.
- Dockerfile multi-stage equivalente y compose de desarrollo apuntando al
  Dockerfile del paquete.
- Pendiente: smoke en `staging` antes del merge a `develop`.
