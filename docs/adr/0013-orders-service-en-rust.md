# ADR-0013: Reescritura de `orders-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-13
- **Estado**: Aceptado — en implementación
- **Ámbito**: `orders-service` (HTTP público e interno, PostgreSQL, RabbitMQ, Outbox, scheduler, métricas)
- **Referencias**: [ADR-0004](./0004-scheduling-service-en-rust.md), [ADR-0006](./0006-notifications-service-en-rust.md), [ADR-0010](./0010-logistics-service-en-rust.md)

## Contexto

`orders-service` es el bounded context de pedidos: CQRS + Event Sourcing sobre
`orders.eventos`, con proyecciones (`orden_vista`, `orden_timeline`,
`comisiones_vista`) mantenidas por trigger. Además del checkout, expone carrito,
entregas digitales, contratos con depósito/garantía, suscripciones recurrentes y
salón (mesas/comandas). Es NestJS (~5.000 líneas sin pruebas) con:

- HTTP público `/api/v2/orders`, `/api/v2/carrito`, `/api/v2/contratos`,
  `/api/v2/suscripciones`, `/api/v2/mesas`, `/api/v2/comandas`,
  `/api/v2/pedidos/:id/entregas-digitales` y
  `/api/v2/entregas-digitales/:token`.
- HTTP interno `/internal/orders/...` (transición, lectura, replay e historia).
- Consumo de `orders.resultados` (`stock.reservado`, `stock.fallido`),
  publicación por Outbox (`order.created`, `order.status.updated`,
  `payment.procesado`) y renovación horaria de suscripciones.
- Integraciones síncronas con `stores` (ofertas), `catalog` (stock, precios y
  cotización de reservas), `identity` (capacidades), `scheduling`
  (disponibilidad) y `finance` (crédito).
- `/health` y `/metrics` con las métricas Prometheus compartidas.

Se reescribe en Rust **sin cambiar contratos, modelos, eventos ni reglas**, con
el patrón validado en `scheduling-service` (ADR-0004), `notifications-service`
(ADR-0006) y `logistics-service` (ADR-0010).

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie pública | mismas rutas, métodos y códigos; envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores `{codigo, mensaje, detalles?}` sin `meta` |
| Autorización | mismos roles y permisos; `RolesGuard` + `CapabilityGuard` con las capacidades `catalogo_items`, `cotizaciones`, `ventas_credito`, `fulfillment_*`; clave interna para `/internal` |
| Validación | mismos DTOs y shape `VALIDACION` con `detalles[{campo, restricciones}]`; numéricos, UUID, fechas, regex y límites idénticos |
| Persistencia | mismas tablas, columnas y consultas; las proyecciones siguen siendo triggers de `orders.eventos`; no se escribe `orden_vista` a mano |
| Event Sourcing | `OrderCreatedEvent`/`OrderStatusUpdatedEvent` con el mismo payload, versión optimista y reintentos ante `23505`; replay idéntico |
| Eventos | mismos nombres y payloads; Outbox en la misma transacción del evento; poller de 1 s, lote de 50 |
| Consumidor | `orders.resultados` con `stock.reservado` → `pagada` + `payment.procesado`; `stock.fallido` → `cancelada`; idempotencia por `event_id` |
| Dinero | enteros en centavos; serialización `"1150.00"`; margen 0–90; comisión por defecto 12 % configurable; nunca punto flotante |
| Estados | mismos estados y transiciones por tipo (compra, reserva, cotización, orden de servicio) |
| Reservas | `recurso_id`, `inicia_en`, `termina_en`; cantidad 1; cotización y disponibilidad por API interna |
| Crédito | registro en finance antes de la orden, compensación si falla; una sola empresa por venta a crédito |
| Carrito | expiración de 30 min, sin reserva de stock, máximo 99 por línea, checkout transaccional |
| Contratos | depósito, garantía, retención y movimientos; estados y reglas de fechas |
| Suscripciones | intervalos semanal/mensual/anual; renovación idempotente por reclamación; órdenes recurrentes |
| Salón | mesas por empresa con código único; comandas por estación con estados |
| Fechas | ISO 8601 UTC con milisegundos y `Z` (R-C-4); fechas de contrato `YYYY-MM-DD` |
| Observabilidad | `/health` `{servicio, uptime_seg}`; `/metrics` Prometheus; logs JSON con `request_id`, `servicio` y nivel |
| Runtime | puerto 3004 (`PORT`), mismas variables de entorno, fail-fast si falta `DATABASE_URL` o `RABBITMQ_URL`, cierre ordenado |

### 2. Implementación destino

- Crate en la raíz de `packages/orders-service/`, binario `orders-service`; se
  retira el TypeScript en la conmutación.
- `axum` + `tokio` para HTTP, `sqlx` para PostgreSQL, `lapin` para RabbitMQ,
  `reqwest` para las APIs internas, `serde`/`serde_json`, `uuid`, `chrono`,
  `thiserror` y `urlencoding`.
- Capas: `http/` (router, envelope, validación, rutas), `pedidos/` (vistas,
  event store, carrito, checkout, consumer, entregas), `contratos.rs`,
  `suscripciones.rs`, `salon.rs`, `outbox.rs`, `rabbit.rs`, `auth.rs`, `db.rs`,
  `config.rs` y `state.rs`.
- Inyección explícita por estado compartido (`AppState` con `Arc`), equivalente
  verificable de la DI de NestJS (enmienda de R-MS-3).

### 3. Verificación de paridad

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y `cargo test` verdes.
2. Pruebas PostgreSQL (`tests/pedidos_pg.rs`): creación con evento + outbox +
   proyección, transiciones válidas, replay, idempotencia del consumidor,
   carrito, contratos, suscripciones y salón.
3. Pruebas HTTP (`tests/http_pedidos.rs`): envelope, autorización, validación,
   rutas internas y capacidades.
4. Los OpenAPI versionados no cambian; el binario sirve el mismo documento.
5. Smoke de `staging` en la primera promoción de la imagen (R-CD-12).

## Plan de migración (PRs atómicos)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 1 | Este ADR y el índice | Decisión registrada |
| 2 | Crate Rust con paridad de contrato, pruebas y Dockerfile | `cargo test` y PostgreSQL verdes |
| 3 | Job `rust` en CI con gate de cobertura | Checks verdes |
| 4 | Conmutación: retirar TypeScript, compose, scripts y baseline Node | `npm run build`, reglas y OpenAPI verdes |
| 5 | Documentación de cobertura/auditoría | R-COV-1 actualizado |

## Consecuencias

### Positivas

- Un servicio NestJS menos y menos consumo en reposo.
- Tipado estricto en eventos, dinero y transiciones de estado.
- Reutiliza el patrón y los gates de ADR-0004/ADR-0006/ADR-0010.

### Costes y riesgos

- Es el servicio con más superficie de negocio migrado hasta ahora (checkout,
  contratos, suscripciones y salón); la paridad se apoya en pruebas PostgreSQL.
- El scheduler de suscripciones pasa de `@nestjs/schedule` a un ticker `tokio`
  cada hora con la misma idempotencia.
- Durante la conmutación conviven la imagen TS y la Rust; solo una se despliega.

## Criterios de aceptación

- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y la
  integración PostgreSQL en verde.
- Evento, outbox y proyección en la misma transacción; consumidor idempotente.
- Contratos, suscripciones y salón con las mismas reglas de dominio.
- OpenAPI, salud y métricas conservan el contrato.
- Smoke de `staging` verde en la primera promoción.
