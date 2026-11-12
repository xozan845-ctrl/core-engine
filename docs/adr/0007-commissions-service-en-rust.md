# ADR-0007: Reescritura de `commissions-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-12
- **Estado**: Aceptado — implementación en curso sobre `feat/commissions-rust`; pendiente el smoke de `staging`
- **Ámbito**: `commissions-service` (HTTP público, PostgreSQL, RabbitMQ, Outbox, worker quincenal, métricas)
- **Referencias**: [ADR-0004](./0004-scheduling-service-en-rust.md), [ADR-0006](./0006-notifications-service-en-rust.md), [ADR-37](../decisiones.md)

## Contexto

`commissions-service` es hoy un servicio NestJS (~1.100 líneas de TypeScript)
que expone:

- API pública `/api/v2/**` para el libro del vendedor (`/vendedores/me/ventas`,
  `/vendedores/me/liquidaciones`), el libro de la empresa
  (`/empresarios/me/ventas`, `/empresarios/me/liquidaciones`), el corte manual
  (`POST /admin/liquidaciones/corte`), el pago de una liquidación
  (`POST /admin/liquidaciones/:id/pagar`) y los KPIs de admin
  (`GET /admin/reportes`), con envelope `{data, meta}` (R-C-1).
- Consumo RabbitMQ de `commissions.pedidos` (`payment.procesado`,
  `order.completado`, `devolucion.solicitada`) y `commissions.liquidacion`
  (`comision.acreditada`).
- Cálculo de comisiones por línea (RN-04), compensación por devolución (RN-06),
  pago simulado del MVP, consolidación quincenal (RN-07) y publicación de
  `comision.acreditada` por Outbox.
- Worker de corte los días 1 y 15 a las 00:05 (zona local, por defecto
  `America/Managua`).
- `/health` y `/metrics` con las métricas Prometheus compartidas.

El servicio se despliega en `staging` (ADR-37) y queda fuera de producción por
ahora. Se decide reescribirlo en Rust **sin cambiar contratos, modelos ni
lógica**, siguiendo el patrón validado en `scheduling-service` (ADR-0004) y
`notifications-service` (ADR-0006). El servicio conserva nombre, puerto,
variables de entorno, esquema de base de datos, topología de eventos y
respuestas HTTP.

El trabajo se desarrolla en la rama de larga vida `feat/commissions-rust`, con
PRs atómicos apilados (R-PR-2). **Esa rama no se mergea a `develop` hasta que la
paridad esté demostrada y el servicio Rust haya pasado un smoke en `staging`.**

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie pública | mismas rutas y métodos; `GET` responde `200`, `POST` responde `201`; envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores `{codigo, mensaje, detalles?}` sin `meta` |
| Autorización | mismo orden de guardas: rol del endpoint (`vendedor`, `empresario`, `super_admin`, `admin`) y capacidad `liquidaciones` consultada a identity; `super_admin`/`admin` omiten la capacidad; sin contexto `401 NO_AUTORIZADO`; rol incorrecto `403 ACCESO_DENEGADO`; sin tenant `400 TENANT_REQUERIDO`; identity caído `503 SERVICIO_NO_DISPONIBLE`; capacidad ausente `403 CAPACIDAD_NO_ACTIVA` |
| Validación | sin DTOs de cuerpo; `inicio`/`fin` y `estado` viajan como query strings; un UUID inválido en `:id` termina en `500 ERROR_INTERNO` (mismo comportamiento que Postgres en el TS) |
| Persistencia | mismas tablas, columnas y consultas: `commissions.comisiones`, `pagos`, `liquidaciones`, `liquidaciones_empresas`, `compensaciones_devoluciones`, `eventos_procesados`, `outbox`; idempotencia por `(order_id, linea_clave)` y por `event_id` |
| RLS | `set_config('request.jwt.claims', …, true)` dentro de la transacción con `sub`, `rol` y `tenant_id`; el pool no usa `BYPASSRLS` |
| Cálculo | RN-04 con la misma tasa (`COMMISSION_RATE`, por defecto `0.12`), redondeo `Math.round` y reparto neto por participación; participación `0`/`100` aceptada; fuera de `0..100` responde `PARTICIPACION_INVALIDA` |
| Errores de dominio | conservan código y status: `MONTO_INVALIDO`, `TASA_INVALIDA`, `PARTICIPACION_INVALIDA`, `COMISION_FALLIDA`, `PAGO_FALLIDO`, `NO_ENCONTRADO`, `TENANT_REQUERIDO` |
| Eventos | mismos nombres y payloads; `payment.procesado` registra el pago, `order.completado` acredita cada línea y emite `comision.acreditada`, `devolucion.solicitada` compensa; `comision.acreditada` en `commissions.liquidacion` solo deja traza |
| Datos externos | `orders` para la orden completada, `catalog` e `identity` para los KPIs; `x-internal-key` y timeout `INTERNAL_REQUEST_TIMEOUT_MS`; fallo de inventario/vendedores degrada a cero; fallo de la orden se propaga para reintento |
| Outbox | misma tabla (`OUTBOX_TABLA`), poller de 1 s, lote de 50, `FOR UPDATE SKIP LOCKED`, `pendiente→publicado` y métricas de outbox |
| RabbitMQ | exchanges `core-engine.events` y `core-engine.dlq`; colas `commissions.pedidos` y `commissions.liquidacion` con sus routing keys; prefetch 1; `x-intentos` máximo 3; DLQ con republicación cada 10 s y supervisor con backoff |
| Worker | corte quincenal los días 1 y 15 a las 00:05 locales; cierra el periodo anterior (día 1 → tramo 16..fin del mes previo; día 15 → 1..15; día 16+ → 16..fin) |
| Observabilidad | `/health` `{servicio, uptime_seg}`; `/metrics` Prometheus con los mismos nombres y etiquetas `{servicio, metodo, ruta, estado}`; logs JSON con `request_id`, `servicio` y nivel |
| Fechas | `timestamptz` ISO 8601 UTC con milisegundos y `Z`; columnas `date` como medianoche local y luego ISO UTC (R-C-4), equivalente a node-postgres |
| Runtime | puerto 3006 (`PORT`), mismas variables de entorno, fail-fast si falta `DATABASE_URL` o `RABBITMQ_URL`, cierre ordenado ante `SIGTERM`/`SIGINT` |
| OpenAPI | `docs/openapi/commissions-service/*.json` se conservan como golden; el binario sirve el documento público sin cambios |

Cualquier diferencia observable en estos puntos se considera regresión y bloquea
el merge de la rama de migración.

### 2. Implementación destino

- Crate en `packages/commissions-service/rust/` durante la migración, con
  binario `commissions-service`. En el PR de conmutación se mueve a la raíz del
  paquete y se retira el código TypeScript.
- `axum` + `tokio` para HTTP, `sqlx` (Postgres, runtime Tokio) para datos,
  `lapin` para RabbitMQ, `reqwest` para las APIs internas, `serde`/`serde_json`,
  `uuid`, `chrono`, `chrono-tz`, `thiserror` y `async-trait`.
- Estructura interna por capas (R-MS-2 adaptada):
  `http/` (router, envelope, errores, auth, métricas), `db.rs` (pool y RLS),
  `comisiones.rs`, `liquidaciones.rs`, `ventas.rs`, `reportes.rs` (dominio y
  repositorios con SQL explícito), `pedidos.rs` (consumidor), `outbox.rs`,
  `rabbit.rs` y `worker.rs` (corte quincenal).
- Inyección de dependencias explícita por estado compartido (`AppState` con
  `Arc` y traits de puerto), equivalente verificable de la DI de NestJS
  (enmienda de R-MS-3).

### 3. Verificación de paridad

1. **Caracterización**: los tests actuales del TS se traducen a pruebas Rust de
   cálculo, periodos, idempotencia, compensación y consolidación.
2. **Integración PostgreSQL**: `tests/comisiones_pg.rs` crea el esquema
   `commissions` y verifica acreditación idempotente, compensación y corte
   quincenal contra una BD efímera.
3. **Contrato OpenAPI**: el documento público versionado se sirve sin cambios y
   una prueba falla si el binario no lo expone.
4. **HTTP**: pruebas con `tower::ServiceExt` para envelope, roles, capacidad,
   errores y códigos `200/201`.
5. **Smoke real**: `scripts/tools/smoke-remote.mjs` (incluye
   `GET /api/v2/empresarios/me/ventas`, ADR-37) y `npm run demo` con el stack
   completo; el PR final a `develop` queda en borrador hasta el smoke de
   `staging`.
6. **Doble corrida controlada**: despliegue del contenedor Rust en `staging` en
   paralelo al TS sobre la misma BD y smoke contra ambos antes de conmutar.

## Plan de migración (PRs atómicos apilados sobre `feat/commissions-rust`)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 0 | Este ADR y el índice | Decisión registrada; sin cambios de código |
| 1 | Andamiaje Rust: configuración, logs, errores, métricas, envelope, auth, `/health`, `/metrics`, `/docs` y job `rust` de CI | `cargo fmt --check`, `cargo clippy -D warnings` y `cargo test` verdes |
| 2 | Dominio de comisiones: `Money`, cálculo RN-04, acreditación, compensación, pagos y consultas | Pruebas unitarias de cálculo, idempotencia y errores |
| 3 | Liquidaciones, ventas y reportes: periodos RN-07, consolidación, estado de cuenta y KPIs | Pruebas unitarias + integración PostgreSQL |
| 4 | Superficie HTTP completa con roles/capacidad, envelope y OpenAPI golden | Contrato HTTP verificado |
| 5 | Consumidor RabbitMQ, Outbox, DLQ y worker quincenal | Pruebas de evento y DLQ verificadas |
| 6 | Imagen Docker Rust y gates de CI (build, cobertura y mutación) | Imagen equivalente; enmiendas registradas |
| 7 | Conmutación: retirar TypeScript, mover el crate a la raíz, actualizar scripts, compose, baseline y suites que importan módulos TS | `npm run demo`, integración y OpenAPI verdes con el servicio Rust |
| 8 | PR final `feat/commissions-rust → develop` | Paridad + smoke de `staging`; recién entonces se mergea |

Los PR 0-7 se mergean a `feat/commissions-rust` por `--rebase` (R-PR-7); el PR 8
permanece en borrador hasta cumplir la verificación de paridad.

## Enmiendas de gobernanza necesarias

- **R-CI-1**: añadir `commissions-service` a la matriz del job `rust` (fmt,
  clippy, test y cobertura) y actualizar la ruta del crate durante la
  migración.
- **R-COV-1/R-COV-2**: la cobertura se mide con `cargo-llvm-cov`; el requisito
  de spec por archivo se traduce a `#[cfg(test)]` + pruebas de integración. Se
  registra el baseline medido y se conserva el umbral 94/77/88 como objetivo
  hasta la primera medición real.
- **R-MS-3**: la DI obligatoria vía framework se interpreta como DI explícita
  verificable en Rust, no como exigencia de NestJS.
- **R-GC-5**: la verificación del ámbito para este servicio pasa a
  `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y
  `cargo test` (además de `npm run demo` para el flujo completo).
- **R-MT-1/R-MT-2**: al retirar TypeScript, `commissions-service` sale de
  Stryker. La mutación del dominio Rust se cubre con property tests de dinero y
  periodo; se registra `cargo-mutants` como deuda de la siguiente auditoría
  (R-COV-3), sin bajar el umbral del resto de módulos críticos.
- **R-COV-2**: la suite `microservices-ci-contract` deja de exigir
  `*.service.ts`/`*.consumer.ts` a los servicios en `RUST_SERVICES` y pasa a
  exigir su gate Rust.

## Consecuencias

### Positivas

- Un servicio menos en el runtime NestJS y menor consumo en reposo.
- Tipado estricto en dinero, periodos y eventos.
- La migración reutiliza el patrón, los gates y la experiencia de ADR-0004 y
  ADR-0006.

### Costes y riesgos

- Durante la migración conviven dos implementaciones; solo una se despliega.
- La suite de integración `scripts/integration/saga.test.mjs` importa
  `ComisionesService` y `PedidosConsumer` del TS; hay que reescribir esos pasos
  contra HTTP/eventos en el PR de conmutación.
- El detalle de serialización es sensible: `date` como medianoche local,
  `numeric`/`integer`, timestamps con milisegundos y orden de campos.
- El cron depende de la zona horaria del proceso; se fija `TZ` y se prueba el
  cálculo de periodos.
- Un error de paridad en producción afectaría comisiones y liquidaciones; por
  eso la conmutación exige doble corrida y smoke en `staging`.

## Criterios de aceptación

- Las pruebas Rust de cálculo, periodos, idempotencia, compensación y corte
  pasan sin diferencias respecto al comportamiento documentado.
- Los OpenAPI versionados no cambian.
- `smoke-remote` y `npm run demo` verdes con el servicio Rust.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y la
  cobertura Rust en verde en CI.
- `develop` no recibe la migración hasta que el PR 8 tenga paridad + smoke de
  `staging` y los checks verdes en su cabeza (R-PR-6).

## Estado de implementación (2026-11-12)

| PR | Estado | Alcance |
|---|---|---|
| #585 | MERGED | ADR-0007 (este documento) |
| #589 | MERGED | Crate Rust con paridad + job `rust` y gate de cobertura (R-CI-1/R-COV-1) |
| #— | EN CURSO | Conmutación: retirar TypeScript, mover el crate a la raíz, Docker/compose/scripts y saga |

- Cobertura Rust medida con PostgreSQL efímero: **73,29 % líneas / 69,70 %
  funciones** (sin `main.rs`); gate 72/68 con ratchet +5 por release hasta
  94/77/88 (R-COV-1).
- Pendiente: `npm run demo`, `smoke-remote` de `staging` y el PR final a
  `develop` (borrador hasta cumplir el gate).
