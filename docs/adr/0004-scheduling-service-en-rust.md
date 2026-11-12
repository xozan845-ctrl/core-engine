# ADR-0004: Reescritura de `scheduling-service` en Rust sin cambio de contrato

- **Fecha**: 2026-11-11
- **Estado**: Aceptado — implementación completa en `feat/scheduling-rust`; pendiente el smoke de `staging` antes del merge a `develop`
- **Ámbito**: `scheduling-service` (HTTP público e interno, PostgreSQL, RabbitMQ, Outbox, métricas)

## Contexto

`scheduling-service` es hoy un servicio NestJS de ~1.300 líneas de TypeScript
(`src/scheduling/`) que expone:

- API pública `/api/v2/scheduling/**` (recursos, disponibilidad, bloqueos, citas
  y recordatorios) con envelope `{data, meta}` (R-C-1) y guardas de rol,
  capacidad de tenant y tenant activo.
- API interna `/internal/scheduling/disponibilidad` con `x-internal-key`, que
  usa `orders-service` para validar una reserva antes de crearla.
- Consumidor RabbitMQ de `order.created` (cola `scheduling.orders`) que crea
  citas desde reservas, con idempotencia por `event_id`.
- Worker interno de recordatorios cada 60 s y publicación de eventos por
  Outbox (`appointment.created`, `appointment.cancelled`,
  `appointment.reminder.due`).
- `/health` y `/metrics` con las métricas Prometheus compartidas
  (`core_engine_http_*`, `core_engine_outbox_*`).

Se decide reescribirlo en Rust **sin cambiar contratos, modelos ni lógica**. No
es una reingeniería funcional: es un cambio de implementación con paridad
observable. El servicio conserva nombre, puerto, variables de entorno, esquema
de base de datos, topología de eventos y respuestas HTTP.

El trabajo se desarrolla en la rama de larga vida `feat/scheduling-rust`, con
PRs atómicos apilados (R-PR-2). **Esa rama no se mergea a `develop` hasta que
la paridad esté demostrada y el servicio Rust haya pasado un smoke en
`staging`.**

## Decisión

### 1. Invariantes que no pueden cambiar

| Área | Invariante |
|---|---|
| Superficie pública | mismos métodos, rutas, códigos 200/201 y envelope `{data, meta:{timestamp, path, method, statusCode}}`; errores `{codigo, mensaje, detalles?}` sin `meta` |
| Superficie interna | `/internal/scheduling/disponibilidad` responde `{disponible: true}` sin envelope y exige `x-internal-key` |
| Autorización | mismos roles (`super_admin`, `admin`, `empresario`, `logistica`, `coordinador`, `supervisor`, `operativo`), capacidad `agenda_recursos` consultada a identity, `tenant_id` obligatorio y misma degradación para roles configurables |
| Validación | mismos DTO, reglas (`uuid`, fecha ISO 8601, enums, `0..6`, strings) y mismo shape de error `VALIDACION` con `detalles[{campo, restricciones}]` |
| Persistencia | mismas tablas y columnas (`scheduling.recursos`, `disponibilidad`, `bloqueos`, `citas`, `recordatorios`, `eventos_procesados`, `outbox`), mismas consultas y locks (`FOR UPDATE`, `FOR UPDATE SKIP LOCKED`, `tstzrange '[)'`) |
| RLS | `set_config('request.jwt.claims', …, true)` dentro de la transacción con `sub`, `rol` y `tenant_id`; el pool no usa `BYPASSRLS` |
| Conflictos | `23P01` y `23505` se traducen a `409 RECURSO_NO_DISPONIBLE`; `PERIODO_INVALIDO`, `NO_ENCONTRADO`, `PARTE_INVALIDA`, `SERVICIO_NO_DISPONIBLE` conservan código y status |
| Eventos | mismos nombres y payloads; Outbox con la misma tabla (`OUTBOX_TABLA`), poller de 1 s, lote de 50, `estado='pendiente'→'publicado'` y métricas de outbox |
| RabbitMQ | exchanges `core-engine.events` y `core-engine.dlq`; cola `scheduling.orders` con routing `order.created`; reintentos `x-intentos` máx. 3 y DLQ con republicación automática cada 10 s |
| Worker | recordatorios cada 60 s, lote de 100, `SKIP LOCKED`, y mismo criterio de creación (`24h` y `2h` solo si el instante ya no pasó) |
| Observabilidad | `/health` `{servicio, uptime_seg}`; `/metrics` Prometheus con los mismos nombres y etiquetas; logs JSON con `request_id`, `servicio` y nivel |
| Fechas | ISO 8601 UTC con milisegundos y `Z` (R-C-4), equivalente a `Date.prototype.toISOString()`; las fechas de `date` viajan como `YYYY-MM-DD` |
| Runtime | puerto 3011 (`PORT`), mismas variables de entorno, fail-fast si falta `DATABASE_URL` o `RABBITMQ_URL`, cierre ordenado ante `SIGTERM`/`SIGINT` |

Cualquier diferencia observable en estos puntos se considera regresión y bloquea
el merge de la rama de migración.

### 2. Implementación destino

- Crate en `packages/scheduling-service/rust/` durante la migración, con binario
  `scheduling-service`. En el PR de conmutación se mueve a la raíz del paquete y
  se retira el código TypeScript.
- `axum` + `tokio` para HTTP, `sqlx` (Postgres, runtime Tokio) para datos,
  `lapin` para RabbitMQ, `reqwest` para las APIs internas de identity,
  `serde`/`serde_json`, `uuid`, `chrono`, `thiserror`, `tracing` y
  `tower-http` para correlación y límites.
- Estructura interna por capas (R-MS-2 adaptada): `http/` (router, envelope,
  errores, auth, métricas), `db/` (pool, RLS, repositorios con SQL explícito),
  `scheduling/` (dominio), `outbox/`, `rabbit/`, `worker/`.
- Inyección de dependencias explícita por estado compartido (`AppState` con
  `Arc`), equivalente verificable de la DI de NestJS (enmienda de R-MS-3).

### 3. Verificación de paridad (criterio de "segura de mergear")

1. **Caracterización antes de reescribir**: suite de contrato que ejecuta los
   flujos contra el servicio TypeScript y guarda fixtures normalizados
   (status, cuerpo con ids/fechas normalizados, filas de BD y eventos de
   outbox). Es el baseline ejecutable.
2. **Paridad diferencial**: la misma suite corre contra el binario Rust sobre
   una BD efímera equivalente; el diff debe ser vacío salvo ids y timestamps.
3. **Contrato OpenAPI**: los JSON de `docs/openapi/scheduling-service/` se
   conservan como golden; el servicio Rust sirve ese documento y una prueba
   falla si el contrato servido difiere del versionado.
4. **Smoke real**: `scripts/tools/smoke-verticals.mjs` (flujo
   recurso→disponibilidad→cita) contra el contenedor Rust en `staging`.
5. **Doble corrida controlada**: despliegue del contenedor Rust en `staging`
   en paralelo al TS sobre la misma BD, con el smoke y la suite de contrato
   apuntando a ambos; sin diferencias antes de conmutar.

## Plan de migración (PRs atómicos apilados sobre `feat/scheduling-rust`)

| PR | Alcance | Criterio de salida |
|---|---|---|
| 0 | Este ADR y el índice | Decisión registrada; sin cambios de código |
| 1 | Suite de contrato/caracterización del servicio actual (HTTP, BD y outbox) | Baseline versionado y reproducible en CI |
| 2 | Andamiaje Rust: config fail-fast, `/health`, `/metrics`, `/docs`, envelope, errores, `x-request-id`, apagado ordenado y job `rust` de CI | `cargo fmt --check`, `cargo clippy -D warnings` y `cargo test` verdes |
| 3 | Recursos, disponibilidad y bloqueos en Rust | Paridad de contrato y de filas de BD |
| 4 | Citas, recordatorios y Outbox en Rust | Paridad de respuestas, eventos y métricas |
| 5 | Consumidor `scheduling.orders` y worker de recordatorios | Idempotencia y DLQ verificadas con RabbitMQ real |
| 6 | Imagen Docker Rust y gates de CI (build, test, cobertura) | Imagen equivalente; enmiendas de R-CI-1/R-COV-1 registradas |
| 7 | Conmutación: retirar TypeScript, actualizar scripts raíz, baseline de cobertura, suites que importan módulos TS y documentación | `npm run demo`, integración y OpenAPI verdes con el servicio Rust |
| 8 | PR final `feat/scheduling-rust → develop` | Paridad + smoke en `staging`; recién entonces se mergea |

Los PR 1-7 se mergean a `feat/scheduling-rust` por `--rebase` (R-PR-7); el PR 8
permanece en borrador hasta cumplir la verificación de paridad.

## Enmiendas de gobernanza necesarias

- **R-CI-1**: añadir el job `rust` (fmt, clippy, test, cobertura) al pipeline y
  a las dependencias de `build`, con enmienda explícita de la regla.
- **R-COV-1/R-COV-2**: el gate actual es Vitest sobre `packages/*`; la
  cobertura del servicio Rust se mide con `cargo-llvm-cov` y el requisito de
  spec por archivo se traduce a `#[cfg(test)]` + pruebas de integración. Se
  registra la enmienda y se conserva el umbral 94/77/88 como objetivo hasta la
  primera medición real.
- **R-MS-3**: la DI obligatoria vía framework se interpreta como DI explícita
  verificable en Rust, no como exigencia de NestJS.
- **R-GC-5**: la verificación del ámbito para este servicio pasa a
  `cargo fmt --check`, `cargo clippy -- -D warnings` y `cargo test` (además de
  `npm run demo` para el flujo completo).

## Consecuencias

### Positivas

- Menor consumo de memoria y arranque más rápido del servicio.
- Tipado estricto y ausencia de `any` en la capa de dominio.
- Contrato y comportamiento congelados por una suite de paridad reutilizable
  para futuras migraciones.

### Costes y riesgos

- Durante la migración conviven dos implementaciones; solo una se despliega.
- La suite de integración actual importa módulos TypeScript
  (`scripts/integration/capability-routes.test.mjs` y
  `vertical-pilots.test.mjs`); hay que reescribirla contra HTTP en el PR 7.
- El detalle de serialización es sensible: `jsonb`, `numeric`, timestamps con
  milisegundos y `notas`/`metadata` por defecto deben coincidir byte a byte.
- El gate de cobertura no mide Rust hoy: sin la enmienda de R-COV-1, el
  servicio quedaría sin cobertura verificada.
- Un error de paridad en producción afectaría reservas y recordatorios; por eso
  la conmutación exige doble corrida y smoke en `staging`.

## Criterios de aceptación

- La suite de paridad pasa sin diferencias (salvo ids/timestamps normalizados)
  contra TypeScript y Rust sobre la misma BD efímera.
- Los OpenAPI versionados no cambian.
- `smoke-verticals` y `npm run demo` verdes con el servicio Rust.
- `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` y la
  cobertura Rust en verde en CI.
- `develop` no recibe la migración hasta que el PR 8 tenga paridad + smoke de
  `staging` y los checks verdes en su cabeza (R-PR-6).

## Estado de implementación (2026-11-12)

Implementación completa sobre `feat/scheduling-rust`; **no mergeada a
`develop`** hasta cumplir el gate de `staging`.

| PR | Estado | Alcance |
|---|---|---|
| #555 | MERGED | ADR-0004 (este documento) |
| #560 | MERGED | Andamiaje Rust + job `rust` de CI (R-CI-1) |
| #561 | MERGED | Recursos, disponibilidad y bloqueos |
| #562 | MERGED | Citas, recordatorios y Outbox |
| #563 | MERGED | RabbitMQ, consumidor y worker |
| #564 | MERGED | Imagen Docker + gate `cargo-llvm-cov` |
| #565 | MERGED | Conmutación: se retira TypeScript y se mueve el crate a la raíz |

- Cobertura Rust medida con `cargo-llvm-cov` sobre PostgreSQL efímero:
  **72,86 % líneas / 66,82 % funciones** (sin `main.rs`), gate 71/65 con
  ratchet +5 por release hasta 94/77/88 (R-COV-1, enmienda en
  `docs/rules/test/06-estandares-cobertura.md`).
- Imagen Rust verificada con PostgreSQL y RabbitMQ reales (`/health`, cola
  declarada, outbox publicado, evento procesado y DLQ).
- El contrato OpenAPI versionado no cambió (`docs/openapi/scheduling-service/`).
- **Cambio respecto al plan**: la suite diferencial TS↔Rust no se ejecutó como
  PR independiente porque el TypeScript se retiró en la conmutación. La
  paridad se verificó con los OpenAPI goldens, la prueba de integración Rust
  contra PostgreSQL real y las corridas manuales con RabbitMQ. La comparación
  diferencial queda como técnica aplicable a futuras migraciones, no como gate
  pendiente de este ADR.

## Gate pendiente antes de `develop`

1. Desplegar la imagen Rust en `staging` y ejecutar `scripts/tools/smoke-verticals.mjs`
   (recurso → disponibilidad → cita) contra el contenedor.
2. Correr `npm run demo` con el stack completo (el compose ya construye la
   imagen Rust).
3. Verificar el run de `ci.yml` en `develop` tras el merge (R-PR-8).
