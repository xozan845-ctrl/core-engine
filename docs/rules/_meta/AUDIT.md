# AUDIT — Estado actual de testing vs `rules/test/`

> **Snapshot** reescrito completo en cada auditoría (R-COV-3): este archivo siempre
> describe el estado **presente** de **este repo (Core Engine)**. El histórico de
> auditorías vive en [`AUDIT-HISTORY.md`](./AUDIT-HISTORY.md); las entradas
> anteriores a 2026-10-01 pertenecen al proyecto origen y se conservan como
> procedencia.

- **Última auditoría de testing:** 2026-11-10 — **16ª de Core Engine** (cierre de robustez R-RB-3/R-RB-4 sobre `develop` `0c19300`).
- **Estado global:** 🟢 **CUMPLE** en los gates automáticos de PR. G-1, G-3, G-4, G-7 y G-8 verdes; G-5 y G-6 siguen parciales por diseño. Deudas restantes: envelope `{data,meta}`, acoplamiento entre esquemas, property/fuzz de fechas y 12 moderates de tooling.

## Resumen por capa

| Capa | Estado | Evidencia |
|---|---|---|
| Unit backend | 🟢 | **1.419 tests / 140 archivos** en verde en **13 workspaces**; `npm test` exit 0 con cobertura |
| Unit frontend | N/A | No hay frontend en el repo (G-2 y R-COV-5 **reservados, nunca reutilizados**) |
| Integración (API+BD) | 🟢 | **16 archivos / 50 tests** con PostgreSQL efímero; cubren stock, contratos, tipos de orden, scheduling, packs v1/v2, RLS entre verticales, enforcement por ruta, flujos piloto y **concurrencia real del Outbox** |
| Contrato (API) | 🟢/🟡 | OpenAPI versionado y sincronizado (G-8); errores 400/401/403/404/409/500 con prueba runtime; envelope `{data,meta}` de R-C-1 sigue pendiente (G-5 parcial) |
| E2E | 🟡 | `e2e.yml` manual/tag ejecuta `npm run demo` y verifica logs R-E-8; cubre R-E-1..7, no es gate de cada PR |
| OpenAPI (spec) | 🟢 | **11 servicios, 338 operaciones: 308 públicas y 30 internas**; `swagger:export` sin diff (G-8) |
| Property-based | 🟡 | `fast-check` cubre dinero, ciclo de orden y parsers en `shared`; faltan propiedades de fechas |
| Mutation testing | 🟢 | Stryker con `break:90` en cinco módulos críticos; mensual/pre-release, no gate de PR |
| Fuzz / robustez de entradas | 🟢/🟡 | Parsers, concurrencia del Outbox, DLQ/reconexión y timeouts internos cubiertos; faltan límites de payload y fallos de broker más amplios |
| Lint (G-7) | 🟢 | **0 errores, 44 warnings** `no-explicit-any` |
| Build (G-7) | 🟢 | shared + gateway + 11 servicios: exit 0 |
| Security gate (R-QA-6) | 🟡 | `npm audit --audit-level=high` → 0; `--audit-level=moderate` → 12 (tooling; sin fix seguro sin bajar ESLint 10→9) |
| Motor de gobernanza | 🟢 | `rules:check`: 60/60 tests, 11 suites, **42/238 IDs ejecutables**; `test:engine` 21; `test:smoke` 61; baselines sin retroceso |

## Cobertura por workspace (ratchet R-COV-1, medición 2026-11-10)

| Workspace | Suites | Tests | Líneas | Ramas | Funciones | Threshold (L/B/F) | Gate |
|---|---|---|---|---|---|---|---|
| `shared` | 24 | 200 | 91,73 | 78,67 | 92,13 | 39 / 39 / 55 | ✅ |
| `api-gateway` | 7 | 120 | 85,71 | 79,06 | 72,91 | 50 / 51 / 57 | ✅ |
| `orders-service` | 9 | 91 | 88,74 | 74,75 | 84,67 | 32 / 23 / 33 | ✅ |
| `scheduling-service` | 3 | 14 | 95,96 | 81,81 | 91,48 | 94 / 77 / 88 | ✅ |
| `commissions-service` | 10 | 47 | 83,17 | 68,75 | 79,71 | 20 / 14 / 22 | ✅ |
| `finance-service` | 19 | 150 | 92,71 | 79,56 | 90,96 | 70 / 52 / 72 | ✅ |
| `market-intelligence-service` | 8 | 42 | 47,97 | 38,65 | 59,72 | 18 / 11 / 35 | ✅ |
| `catalog-service` | 12 | 155 | 95,63 | 83,68 | 95,96 | 93 / 82 / 90 | ✅ |
| `identity-service` | 22 | 241 | 93,10 | 83,30 | 93,91 | 89 / 78 / 85 | ✅ |
| `stores-service` | 10 | 163 | 93,99 | 86,25 | 94,82 | 86 / 84 / 76 | ✅ |
| `logistics-service` | 3 | 53 | 100,00 | 99,04 | 100,00 | 99 / 96 / 99 | ✅ |
| `field-service` | 2 | 24 | 86,10 | 73,14 | 74,76 | 84 / 70 / 72 | ✅ |
| `storage-service` | 11 | 119 | 100,00 | 98,51 | 100,00 | 99 / 86 / 99 | ✅ |
| **Total** | **140** | **1.419** | — | — | — | 13 workspaces | **13/13 ✅** |

- **Objetivo final (80 líneas / 70 ramas): 12 de 13 workspaces lo alcanzan**. Solo queda fuera `market-intelligence-service` (47,97/38,65), que sube por ratchet (+5/release).
- **`shared` superó el objetivo** tras cubrir `OutboxService`, `PgService`, `service-clients` y los timeouts internos (91,73/78,67/92,13).
- `npm run coverage:compare`: **13/13 workspaces iguales o por encima del baseline**.
- Método: baseline medido redondeado a la baja; los thresholds viven en `scripts/config/coverage.mjs`.

## Métricas actuales vs mínimos

| Métrica | Valor actual | Mínimo (regla) | Cumple |
|---|---|---|---|
| Unit tests (13 workspaces) | 1.419/1.419 en verde | 100% (G-1) | ✅ |
| Cobertura ≥ `coverageThreshold` | 13/13 workspaces | floor medido (R-COV-1) | ✅ |
| Cobertura objetivo final | 12/13 ≥ 80/70 | 80/70 | ⚠️ ratchet +5/release |
| Controllers con spec | **41 de 41** | R-U-10/11 (al tocar endpoints) | ✅ |
| Endpoints documentados (OpenAPI) | **338/338** en 11 servicios (308 públicas + 30 internas) | 100% (R-DO-6, R-DO-10, G-8) | ✅ |
| `*.service.ts` / `*.consumer.ts` sin spec | **0 de 54** | 0 (R-U-18, R-COV-2) | ✅ |
| Código puro de `shared` con spec (R-U-17) | 7 de 7 | 0 sin spec | ✅ |
| Guards/pipes con spec (R-U-15) | `service-auth`, `validation.pipe` | 0 sin spec | ✅ |
| Tests integración | 50 tests en 16 archivos | ≥1 por flujo crítico API+BD (R-I-1) | ✅ G-4 |
| Tests contrato | 6 runtime de errores + OpenAPI | ≥1 por shape (R-C-10) | ✅/🟡 envelope |
| Tests E2E | workflow manual/release con `npm run demo` | R-E-1..7 | ⚠️ parcial G-6 |
| Property-based | 3 suites en `shared` | R-PB-1..3 | ⚠️ parcial fechas |
| Mutación | 5 módulos con `break:90` | ≥ 90% (R-MT-2) | ✅ 5/5 |
| Lint | 0 errores / 44 warnings | 0 errores (G-7) | ✅ |
| Build | 0 errores TS | 0 errores (G-7) | ✅ |
| Audit `high` | exit 0 | 0 (R-QA-6) | ✅ |
| Audit `moderate` | **12** (tooling) | 0 (objetivo, no gate) | ⚠️ |
| Motor de reglas | 60 tests / 11 suites; 42/238 IDs | R-CI-7 | ✅ |
| Baselines | coverage/specs/rls/migrations sin retroceso | `baseline:guard` | ✅ |
| Migraciones | 61 íntegras y registradas | R-DB-6 | ✅ |
| RLS | 59 tablas; ninguna nueva sin RLS | R-SB-2 | ✅ |

## Gates de CI (verificados contra `.github/workflows/ci.yml`)

| Gate | Estado | Evidencia |
|---|---|---|
| G-1 Unit backend | ✅ | `npm test` exit 0; 13/13 workspaces; 1.419 tests |
| G-2 Unit frontend | N/A | No hay frontend (ID reservado) |
| G-3 Cobertura ≥ ratchet | ✅ | 13/13 sobre threshold y sobre baseline |
| G-4 Integración | ✅ | 50 tests en 16 archivos; matriz multi-vertical, packs, RLS, enforcement, flujos piloto y concurrencia del Outbox. La saga asíncrona completa sigue en E2E |
| G-5 Contrato | ✅/🟡 | OpenAPI sincronizado y errores normalizados; envelope `{data,meta}` pendiente de `/api/v2`. No hay gate de breaking: el contrato se regenera y revisa por G-8 |
| G-6 E2E de flujos | ✅/🟡 | `e2e.yml` release/manual; no corre en cada PR |
| G-7 Lint **y** build | ✅ | 0 errores / 44 warnings; build 0 |
| G-8 Swagger sync | ✅ | `swagger:export` sin diff (338 operaciones) |

**Total: 5 ✅ · 1 N/A · 2 ✅/🟡 parciales por diseño.** Ningún gate automático de PR está rojo.

## Correcciones aplicadas en el cierre (2026-11-10)

### Seguridad y autorización

- **Gateway**: añadidas políticas para `POST /auth/cambiar-contrasena` y
  `POST /auth/vincular-personal` (antes 403 por deny-by-default).
- **Market intelligence**: `/me/*` exige `x-user-personal` y ya no devuelve
  datos de todos los vendedores; la ingesta valida DTO, fuerza el vendedor del
  token y limita los roles de borde; agregación idempotente por `event_id`/
  `order_id` en una transacción.
- **Secretos**: `JWT_SECRET`, `ADMIN_PASSWORD` y `ADMIN_EMAIL` fallan al
  arrancar en producción si faltan (defaults solo en desarrollo).

### Transaccionalidad y concurrencia

- **Outbox**: reclamo y publicación dentro de la misma transacción con
  `FOR UPDATE SKIP LOCKED`; sin ventana de 30 minutos y con test de concurrencia
  real de dos pollers contra PostgreSQL (`scripts/integration/outbox.test.mjs`).
- **Event store**: índice único `(aggregate_id, version)` y reintento con
  savepoint ante `23505`.
- **Saga**: stock + outbox + idempotencia en una transacción (catalog);
  `payment.procesado` en la misma transacción que la transición a `pagada`
  (orders); comisiones y outbox en una transacción (commissions).
- **RLS**: políticas para `orders.orden_vista` (la proyección real) y contexto
  de sistema en los consumidores de logistics/stores/finance/orders/market.
- **Storage**: cuota revalidada bajo lock de la fila del bucket; borrado lógico
  primero y MinIO después, tolerando fallos del almacén.
- **Scheduling**: bloqueo y cita serializados con `FOR UPDATE` del recurso;
  `23505` traducido a conflicto.

### Robustez de broker y timeouts (R-RB-3/R-RB-4)

- **RabbitMQ**: JSON inválido va a DLQ marcado como muerto; la DLQ se confirma
  antes de hacer `ack` del original; los mensajes muertos no se reencolan en
  bucle; reconexión con backoff 1s→30s y parada limpia.
- **Timeouts internos**: `timeoutInternoMs()` se exporta desde
  `shared/src/clients/service-clients.ts` y se reutiliza en todos los `fetch`
  internos de catalog, commissions, finance, logistics, orders y stores.
- **Degradación controlada**: `logistics.avanzarEstadoOrden` responde
  `SERVICIO_NO_DISPONIBLE` si orders no responde; el resto mantiene sus
  retornos degradados (`null`, listas vacías o valores por defecto).
- **Gobernanza**: el check `R-RB-4` de `microservices-ci-contract` falla si un
  `fetch` interno no declara timeout (`AbortSignal.timeout` o el patrón
  `AbortController` del gateway).

### Observabilidad

- **Outbox**: gauge `core_engine_outbox_pending_events` y contadores
  `core_engine_outbox_published_events_total` /
  `core_engine_outbox_publish_failures_total`.
- **Dashboards**: `Core Engine - Outbox y DLQ`, con enlace al tablero de
  RabbitMQ para la profundidad de DLQ (sin duplicar la serie).
- **Alertas**: `OutboxBacklog` y `OutboxFallosPublicacion`; la DLQ mantiene
  `RabbitMQDLQCrecimientoSostenido` por crecimiento neto, no por existencia.

### Contabilidad y contrato

- **Comisión**: orders usa `COMMISSION_RATE` y el trigger toma
  `comision_cents` del evento (migración `46_commission_payload.sql`).
- **Finance**: el asiento de realización incluye la parte del empresario
  (cuenta `2.1.6`) y la devolución revierte exactamente las líneas del asiento
  previo.
- **OpenAPI**: `parte_id` y los campos de la ingesta de inteligencia quedan
  opcionales en el spec; el contrato se regenera y `git diff --exit-code` se
  mantiene verde (G-8).

### Tests y tooling

- Specs nuevas: `OutboxService`, `PgService`, `service-clients`, runtime config,
  event store y los controllers de `capacidades`, `extensiones` y `partes`;
  `spec-baseline` vacío (0 servicios/consumers sin spec).
- Integración: `catalog-items` aplica `01c` antes de `01e`; `orders-types`
  aplica las migraciones 45 y 46; el Outbox compite con dos pollers reales.
- QA harness: entrypoint `.js`, `bcryptjs` declarado, sin falsos verdes,
  opciones CLI efectivas y lectura de profundidad de colas.
- Documentación alineada (README, reglas, runbooks, plantillas, QA harness).

## Deudas abiertas que NO son fallos de gate

1. **G-4**: la integración cubre los flujos críticos de PR; la saga asíncrona completa (outbox → broker → consumidor) sigue verificándose en E2E.
2. **G-5**: envelope `{data, meta}` de R-C-1 requiere una migración versionada a `/api/v2`.
3. **G-6**: E2E de release/manual, no en cada PR.
4. **Property/fuzz**: faltan propiedades de fechas y fallos de broker más amplios (R-PB/R-RB parciales).
5. **Acoplamiento entre esquemas**: orders lee `identity.empresa_capacidades`,
   scheduling/field leen `identity.partes` e identity escribe `stores.tiendas`.
   Es una decisión de integración por BD documentada; no se cambió.
6. **Cobertura de capacidades**: solo catalog/stores/scheduling/identity montan
   `CapabilityGuard`; ampliarlo a más servicios es una decisión de producto.
7. **12 moderates** de ESLint/Stryker/typescript-eslint: no hay fix sin bajar
   ESLint 10→9 (major); el gate R-QA-6 solo exige `high`.
8. **44 warnings `no-explicit-any`** y código/colas sin uso (`reproyectar` es un
   no-op, algunas colas no tienen consumidor).

## Evidencia de la auditoría (2026-11-10)

- `npm ci` → 0; `npm run lint` → 0 (44 warnings); `npm run build` → 0.
- `npm test` → 0; **1.419 tests / 140 archivos / 13 workspaces**.
- `npm run coverage:compare` → 0 (13/13 ≥ baseline).
- `npm run swagger:export` → 0 y sin diff; 308 públicas + 30 internas = 338.
- `npm run rules:check` → 0 (60 tests, 11 suites, 42/238 IDs); `test:engine` → 0 (21);
  `test:smoke` → 0 (61).
- `TEST_DATABASE_URL=... npm run test:integration` → 0 (16 archivos / 50 tests,
  incluido el test de concurrencia del Outbox).
- `npm run baseline:guard` → 0; `db:migrations` → 61 íntegras; `db:rls` → 59
  tablas, ninguna nueva sin RLS.
- `npm audit --audit-level=high` → 0; `--audit-level=moderate` → 12.
- **CI post-merge:** run `42908104513` sobre `develop` (`0c19300`) en `success`
  en Rules, Security Gate, ESLint, Unit Tests + Coverage, API + BD contracts y
  Build all services.

## Cómo re-auditar

```bash
npm ci
npm run build:shared
npm test                                  # G-1/G-3
npm run coverage:compare                  # R-COV-1
npm run lint                              # G-7
npm run build                             # G-7
npm run swagger:export && git diff --exit-code docs/openapi docs/api packages/api-gateway/src/openapi.agregado.ts   # G-8
npm run baseline:guard                    # baselines sin retroceso
npm run rules:check                       # motor de gobernanza
npm run test:engine && npm run test:smoke
npm run db:migrations && npm run db:rls
TEST_DATABASE_URL=postgres://core_engine_test:test_only@127.0.0.1:55432/core_engine_test npm run test:integration
npm audit --audit-level=high
```

Al terminar: **reescribir este snapshot** y **agregar una entrada nueva** al final de `AUDIT-HISTORY.md` (R-COV-3).
