# ADR-0010: Reescritura de `logistics-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado — implementación completa; pendiente el smoke de `staging` en la primera promoción de la imagen
- **Ámbito**: `logistics-service` (HTTP público, PostgreSQL, RabbitMQ, Outbox, métricas)
- **Referencias**: [ADR-0004](./0004-scheduling-service-en-rust.md), [ADR-0006](./0006-notifications-service-en-rust.md)

## Contexto

`logistics-service` era un servicio NestJS (~450 líneas de TypeScript) que:

- Expone `/api/v2/orders/:id/estado` (transición de la orden vía orders),
  `/api/v2/admin/envios`, `/api/v2/empresarios/me/envios` (listado y
  asignación de flota) y `/api/v2/envios/:id/estado` (avance por la flota),
  con envelope `{data, meta}` (R-C-1), guardas de rol, capacidad de tenant y
  tenant activo.
- Consume la cola `logistics.pagos` (`payment.procesado` y
  `order.status.updated`), prepara guías de despacho idempotentes por
  `(order_id, empresa_id)` y emite `shipment.started`, `order.completado` y
  `devolucion.solicitada`.
- Deriva la flota de las capacidades `logistica_rutas` y `fulfillment_delivery`
  consultadas a identity, sin leer su esquema.
- Persiste en `logistics.envios`, `logistics.asignaciones_envio`,
  `logistics.eventos_procesados` y `logistics.outbox`.
- Expone `/health` y `/metrics` con las métricas Prometheus compartidas.

Se reescribe en Rust **sin cambiar contratos, modelos ni lógica**, siguiendo el
patrón validado en `scheduling-service` (ADR-0004) y `notifications-service`
(ADR-0006).

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie pública | mismos métodos, rutas y 200; envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores `{codigo, mensaje, detalles?}` sin `meta` |
| Autorización | mismos roles (`super_admin`, `admin`, `empresario`, `logistica`, `coordinador`, `supervisor`, `operativo`), mismas capacidades (`fulfillment_delivery`, `logistica_rutas`), tenant obligatorio y misma degradación para roles configurables |
| Validación | mismos DTO (`AvanzarEstadoRequestDto`, `AsignarProveedorRequestDto`, `AvanzarEnvioRequestDto`), mismos enums y shape `VALIDACION` con `detalles[{campo, restricciones}]` |
| Persistencia | mismas tablas, columnas, índices y consultas (`SELECT *`, `UPDATE … RETURNING *`, límite 200, orden por `creado_en`) |
| Idempotencia | guía única por `(order_id, empresa_id)`; eventos por `event_id` en `logistics.eventos_procesados`; asignación automática única |
| Guía | formato `BGH-<base36(millis)>-<6 uuid en mayúsculas>` |
| Montos | enteros en centavos; `monto` serializado como `"1150.00"`; nunca punto flotante |
| Flota | propias (`logistica_rutas`) + delivery (`fulfillment_delivery`) ⇒ híbrida; solo propias ⇒ `empresa`; resto ⇒ `plataforma` con `PLATFORM_LOGISTICS_TENANT_ID` |
| Transiciones | `enviada` cuenta como `enviada` o `entregada`; la orden solo avanza cuando no quedan envíos pendientes; se propaga el actor |
| Errores | `ENVIO_NO_ENCONTRADO`, `ASIGNACION_FIJA`, `ENVIO_FALLIDO`, `ORDEN_SIN_LINEAS`, `EMPRESA_REQUERIDA`, `TENANT_REQUERIDO`, `ESTADO_INVALIDO`, `ACCESO_DENEGADO`, `NO_ENCONTRADO`, `SERVICIO_NO_DISPONIBLE`, `CAPACIDAD_NO_ACTIVA` |
| Eventos | mismos nombres y payloads; Outbox con poller de 1 s, lote de 50 y `pendiente→publicado` |
| RabbitMQ | exchanges `core-engine.events`/`core-engine.dlq`; cola `logistics.pagos` con routing `payment.procesado` y `order.status.updated`; reintentos `x-intentos` máx. 3 y DLQ con republicación cada 10 s |
| Observabilidad | `/health` `{servicio, uptime_seg}`; `/metrics` Prometheus; logs JSON con `request_id`, `servicio` y nivel |
| Fechas | ISO 8601 UTC con milisegundos y `Z` (R-C-4) |
| Runtime | puerto 3005 (`PORT`), mismas variables de entorno, fail-fast si falta `DATABASE_URL` o `RABBITMQ_URL`, cierre ordenado |

### 2. Implementación destino

- Crate en la raíz de `packages/logistics-service/`, con binario
  `logistics-service`; se retira el TypeScript en la conmutación.
- `axum` + `tokio` para HTTP, `sqlx` para PostgreSQL, `lapin` para RabbitMQ,
  `reqwest` para las APIs internas, `serde`/`serde_json`, `uuid`, `chrono`,
  `thiserror` y `urlencoding`.
- Capas: `http/` (router, envelope, rutas), `envios/` (dominio), `consumidor.rs`,
  `outbox.rs`, `rabbit.rs`, `clients.rs`, `auth.rs`, `db.rs`, `config.rs`.
- Inyección explícita por estado compartido (`AppState` con `Arc`), equivalente
  verificable de la DI de NestJS (enmienda de R-MS-3).

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y
   `cargo test` verdes.
2. Pruebas de integración PostgreSQL (`tests/envios_pg.rs`): idempotencia de la
   guía, derivación de flota, `payment.procesado` → `shipment.started`,
   idempotencia por `event_id`, avance de la orden solo con todos los envíos y
   rechazo de asignación fija.
3. Los OpenAPI versionados no cambian; el binario sirve el mismo documento.
4. Smoke de `staging` en la primera promoción de la imagen (R-CD-12).

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Este ADR y el índice | Decisión registrada |
| 2 | Andamiaje Rust: configuración, HTTP, métricas, envelope y errores | `cargo test` verde |
| 3 | Dominio de envíos, guías, flota y asignación | Integración PostgreSQL verde |
| 4 | Consumidor, Outbox, RabbitMQ y transiciones | Idempotencia y eventos verificados |
| 5 | Imagen Docker, job `rust`, compose y cobertura | Build y gates verdes |
| 6 | Conmutación: retirar TypeScript y actualizar scripts raíz | `npm run demo` y OpenAPI verdes |
| 7 | Smoke de `staging` | Imagen Rust validada antes de producción |

## Consecuencias

### Positivas

- Un servicio NestJS menos y menos consumo en reposo.
- Tipado estricto en montos, clientes y eventos.
- Reutiliza el patrón y los gates de ADR-0004/ADR-0006.

### Costes y riesgos

- `urlencoding` es una dependencia nueva en el inventario Rust.
- La paridad de la guía depende del reloj y del formato base36; queda cubierta
  por pruebas unitarias.
- Durante la conmutación conviven la imagen TS y la Rust; solo una se despliega.

## Criterios de aceptación

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y la
  integración PostgreSQL en verde.
- Guía idempotente, eventos una sola vez y avance de orden con todos los envíos.
- OpenAPI, salud y métricas conservan el contrato.
- Smoke de `staging` verde en la primera promoción.

## Estado de implementación (2026-11-13)

- Crate completo en `packages/logistics-service/` con 55 pruebas unitarias y 4
  de integración PostgreSQL.
- TypeScript retirado; el servicio entra en `RUST_SERVICES` y su gate pasa al
  job `rust` de CI con cobertura 63/62 (medida: 64,57 % líneas / 63,42 %
  funciones).
- Imagen Docker multi-stage equivalente (puerto 3005) y compose de desarrollo
  apuntando al Dockerfile del servicio.
