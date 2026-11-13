# AUDIT — Estado actual de testing vs `rules/test/`

> **Snapshot** reescrito completo en cada auditoría (R-COV-3): este archivo siempre
> describe el estado **presente** de **este repo (Core Engine)**. El histórico de
> auditorías vive en [`AUDIT-HISTORY.md`](./AUDIT-HISTORY.md); las entradas
> anteriores a 2026-10-01 pertenecen al proyecto origen y se conservan como
> procedencia.

- **Última auditoría de testing:** 2026-11-11 — **17ª de Core Engine** (cierre del envelope `{data,meta}` y de los ADR de notificaciones).
- **Estado global:** 🟢 **CUMPLE** en los gates automáticos de PR. G-1, G-3, G-4, G-5, G-6, G-7 y G-8 configurados y verdes; G-2 N/A. Deudas restantes: property/fuzz de fechas y fallos de broker, cobertura de `market-intelligence` y `commissions`, 45 warnings `no-explicit-any` y 12 moderates + 1 low de tooling.

## Resumen por capa

| Capa | Estado | Evidencia |
|---|---|---|
| Unit backend | 🟢 | **1.533 tests / 151 archivos** en verde en **14 workspaces**; `npm test` exit 0 con cobertura |
| Unit frontend | N/A | No hay frontend en el repo (G-2 y R-COV-5 **reservados, nunca reutilizados**) |
| Integración (API+BD) | 🟢 | **20 archivos / 67 tests** con PostgreSQL efímero; cubren stock, contratos, tipos de orden, scheduling, packs v1/v2, RLS, enforcement, pilotos, concurrencia del Outbox, saga completa y FEFO |
| Contrato (API) | 🟢 | Envelope `{data,meta}` en **11/11** servicios; OpenAPI sincronizado; errores 400/401/403/404/409/500 con prueba runtime; check R-C-1 sobre todo el contrato público |
| E2E | 🟢 | `release.yml` ejecuta `e2e.yml` antes de publicar imágenes; `npm run demo` cubre R-E-1..7 y el workflow verifica R-E-8 |
| OpenAPI (spec) | 🟢 | **12 servicios, 359 operaciones: 327 públicas y 32 internas**; `swagger:export` sin diff (G-8) |
| Property-based | 🟡 | `fast-check` cubre dinero, ciclo de orden y parsers; faltan propiedades de fechas |
| Mutation testing | 🟢 | Stryker con `break:90` en cinco módulos críticos; mensual/pre-release, no gate de PR |
| Fuzz / robustez de entradas | 🟢/🟡 | Parsers, concurrencia del Outbox, DLQ/reconexión, timeouts internos y saga cubiertos; faltan límites de payload y fallos de broker más amplios |
| Lint (G-7) | 🟢 | **0 errores, 45 warnings** `no-explicit-any` |
| Build (G-7) | 🟢 | shared + gateway + 12 servicios: exit 0 |
| Security gate (R-QA-6) | 🟡 | `npm audit --audit-level=high` → 0; `--audit-level=moderate` → 13 (1 low, 12 moderate; tooling) |
| Motor de gobernanza | 🟢 | `rules:check`: 90/90 tests, 22 suites, **52/238 IDs ejecutables**; `test:engine` 21; `test:smoke` 61; baselines sin retroceso |

## Cobertura por workspace (ratchet R-COV-1, medición 2026-11-11)

| Workspace | Suites | Tests | Líneas | Ramas | Funciones | Threshold (L/B/F) | Gate |
|---|---|---|---|---|---|---|---|
| `shared` | 26 | 222 | 91,58 | 77,94 | 91,23 | 39 / 39 / 55 | ✅ |
| `api-gateway` | 7 | 121 | 85,71 | 79,06 | 72,91 | 50 / 51 / 57 | ✅ |
| `orders-service` (Rust) | — | 122 | 62,70 | — | 64,35 | 62 / N/A / 63 | ✅ |
| `scheduling-service` | 3 | 14 | 95,96 | 81,81 | 91,48 | 94 / 77 / 88 | ✅ |
| `commissions-service` (Rust) | — | 83 | 73,29 | — | 69,70 | 72 / N/A / 68 | ✅ |
| `finance-service` | 19 | 153 | 88,71 | 74,33 | 89,27 | 70 / 52 / 72 | ✅ |
| `market-intelligence-service` | 8 | 42 | 47,97 | 38,65 | 59,72 | 18 / 11 / 35 | ✅ |
| `catalog-service` | 14 | 174 | 95,74 | 84,03 | 96,01 | 93 / 82 / 90 | ✅ |
| `identity-service` | 25 | 272 | 93,41 | 82,12 | 94,42 | 89 / 78 / 85 | ✅ |
| `stores-service` | 10 | 163 | 94,02 | 86,45 | 94,91 | 86 / 84 / 76 | ✅ |
| `logistics-service` | 3 | 54 | 100,00 | 99,04 | 100,00 | 99 / 96 / 99 | ✅ |
| `field-service` | 2 | 24 | 86,10 | 73,14 | 74,76 | 84 / 70 / 72 | ✅ |
| `storage-service` | 11 | 119 | 100,00 | 98,51 | 100,00 | 99 / 86 / 99 | ✅ |
| `notifications-service` | 4 | 25 | 100,00 | 88,88 | 100,00 | 99 / 85 / 99 | ✅ |
| **Total** | **132** | **1.505** | — | — | — | 10 Vitest + 4 Rust | **14/14 ✅** |

> **Nota (2026-11-12/13)**: `scheduling-service` (ADR-0004),
> `notifications-service` (ADR-0006), `commissions-service` (ADR-0007),
> `field-service` (ADR-0008), `market-intelligence-service` (ADR-0009) y
> `logistics-service` (ADR-0010) y `orders-service` (ADR-0012) migraron a Rust y salen del gate Vitest; su
> cobertura se mide con `cargo-llvm-cov` en el job `rust` de CI. La tabla
> anterior conserva el snapshot histórico; las filas de los servicios Rust se
> actualizan con la medición del job.
- Método: baseline medido redondeado a la baja; los thresholds viven en `scripts/config/coverage.mjs`.

## Métricas actuales vs mínimos

| Métrica | Valor actual | Mínimo (regla) | Cumple |
|---|---|---|---|
| Unit tests (10 workspaces Vitest + 4 Rust) | 1.505/1.505 + Rust en verde | 100% (G-1) | ✅ |
| Cobertura ≥ `coverageThreshold` | 10/10 workspaces + 4 gates Rust | floor medido (R-COV-1) | ✅ |
| Cobertura objetivo final | 9/10 ≥ 80/70 | 80/70 | ⚠️ ratchet +5/release |
| Controllers con spec | **43 de 43** | R-U-10/11 (al tocar endpoints) | ✅ |
| Endpoints documentados (OpenAPI) | **359/359** en 12 servicios (327 públicas + 32 internas) | 100% (R-DO-6, R-DO-10, G-8) | ✅ |
| `*.service.ts` / `*.consumer.ts` sin spec | **0 de 60** | 0 (R-U-18, R-COV-2) | ✅ |
| Código puro de `shared` con spec (R-U-17) | 7 de 7 | 0 sin spec | ✅ |
| Guards/pipes con spec (R-U-15) | `service-auth`, `validation.pipe` | 0 sin spec | ✅ |
| Tests integración | 67 tests en 20 archivos | ≥1 por flujo crítico API+BD (R-I-1) | ✅ G-4 |
| Tests contrato | 6 runtime de errores + OpenAPI + envelope | ≥1 por shape (R-C-10) | ✅ |
| Tests E2E | `release.yml` → `e2e.yml` (R-E-1..7 + R-E-8) | R-E-1..7 | ✅ G-6 |
| Property-based | 3 suites en `shared` | R-PB-1..3 | ⚠️ parcial fechas |
| Mutación | 3 módulos con `break:90` (commissions y orders salen a Rust) | ≥ 90% (R-MT-2) | ✅ 3/3 |
| Lint | 0 errores / 45 warnings | 0 errores (G-7) | ✅ |
| Build | 0 errores TS | 0 errores (G-7) | ✅ |
| Audit `high` | exit 0 | 0 (R-QA-6) | ✅ |
| Audit `moderate` | **13 totales** (1 low, 12 moderate) | 0 (objetivo, no gate) | ⚠️ |
| Motor de reglas | 90 tests / 22 suites; 52/238 IDs | R-CI-7 | ✅ |
| Baselines | coverage/specs/rls/migrations sin retroceso | `baseline:guard` | ✅ |
| Migraciones | 71 íntegras y registradas | R-DB-6 | ✅ |
| RLS | 46 tablas; ninguna nueva sin RLS | R-SB-2 | ✅ |

## Gates de CI (verificados contra `.github/workflows/ci.yml`)

| Gate | Estado | Evidencia |
|---|---|---|
| G-1 Unit backend | ✅ | `npm test` exit 0; 10/10 workspaces Vitest + 4 Rust; 1.505 tests |
| G-2 Unit frontend | N/A | No hay frontend (ID reservado) |
| G-3 Cobertura ≥ ratchet | ✅ | 10/10 Vitest + 4 gates Rust sobre threshold y baseline |
| G-4 Integración | ✅ | 67 tests en 20 archivos; matriz multi-vertical, packs, RLS, enforcement, pilotos, Outbox, saga y FEFO |
| G-5 Contrato | ✅ | Envelope `{data,meta}` en 11/11 servicios; R-C-1 comprobado sobre el OpenAPI público; errores normalizados con prueba runtime |
| G-6 E2E de flujos | ✅ | `release.yml` llama a `e2e.yml` antes de `images`; `npm run demo` + R-E-8 |
| G-7 Lint **y** build | ✅ | 0 errores / 45 warnings; build 0 |
| G-8 Swagger sync | ✅ | `swagger:export` sin diff (359 operaciones) |

**Total: 7 ✅ · 1 N/A.** Ningún gate automático de PR está rojo.

## Correcciones aplicadas en el cierre (2026-11-11)

### Contrato y envelope

- `EnvelopeInterceptor` compartido y aplicado en los **11 servicios** con API pública: `commissions`, `finance`, `orders`, `logistics`, `scheduling`, `field`, `storage`, `market-intelligence`, `catalog`, `stores` e `identity`.
- OpenAPI público de cada servicio envuelto en `{data,meta}`; `/internal`, `/health`, `/metrics`, `/docs` y binarios públicos de storage quedan fuera.
- `smoke`, `smoke-remote` y QA harness desenvuelven el envelope de forma central.
- `openapi-contract` comprueba que toda respuesta pública JSON 2xx tiene `data` y `meta`, y que todo servicio con `/api/v2` monta el interceptor (R-C-1).

### Robustez y concurrencia

- **Outbox**: reclamo y publicación en la misma transacción con `FOR UPDATE SKIP LOCKED`; sin ventana de 30 minutos; test de concurrencia real con dos pollers.
- **RabbitMQ**: JSON inválido a DLQ, confirmación de la DLQ antes del `ack`, mensajes muertos sin bucle y reconexión con backoff 1s→30s.
- **Timeouts internos**: `timeoutInternoMs()` reutilizado en todos los `fetch`; logistics degrada con `SERVICIO_NO_DISPONIBLE`; check R-RB-4 en CI.
- **Observabilidad**: backlog y fallos del Outbox, dashboard y alertas; DLQ sigue con su métrica existente sin duplicarse.

### Seguridad, tenancy y datos

- Gateway con políticas para cambio de contraseña y vinculación de personal.
- Market intelligence aislado por vendedor; ingesta idempotente y roles acotados.
- Secretos obligatorios en producción; RLS para `field` y proyecciones de órdenes.
- Capacidades exigidas en finance, commissions, envíos y rutas de capacidad multi-vertical.
- FEFO con stock, reservas y vencimientos; saga completa cubierta por integración.

### Tests y tooling

- Specs nuevas para Outbox, PgService, service-clients, configuración runtime, event store, controllers de capacidades/extensiones/partes y notificaciones.
- `spec-baseline` vacío: **0 servicios/consumers sin spec**.
- E2E de release como gate obligatorio; smoke y QA con desenvuelto central.
- Documentación, ADR y runbooks alineados.

## Deudas abiertas que NO son fallos de gate

1. **G-6**: el gate de release está configurado; la primera release real debe dejar la evidencia del run E2E.
2. **Property/fuzz**: faltan propiedades de fechas y fallos de broker más amplios (R-PB/R-RB parciales).
3. **Cobertura objetivo**: 10/11 workspaces Vitest alcanzan 80/70; `market-intelligence` (47,97/38,65) sube por ratchet. Los servicios Rust se miden con `cargo-llvm-cov`.
4. **45 warnings `no-explicit-any`** en `packages/*/src`.
5. **13 vulnerabilidades de tooling** (1 low, 12 moderate): no hay fix seguro sin bajar ESLint 10→9; el gate solo exige `high`.
6. **QA harness fuera de ESLint**: `qa-harness/` no está linteado.
7. **Cobertura de capacidades**: se amplió a finance/commissions/envíos, pero no todas las rutas de todos los servicios declaran una capacidad de negocio.

## Evidencia de la auditoría (2026-11-11)

- `npm ci` → 0; `npm run lint` → 0 (45 warnings); `npm run build` → 0.
- `npm test` → 0; **1.533 tests / 151 archivos / 14 workspaces**.
- `npm run coverage:compare` → 0 (14/14 ≥ baseline).
- `npm run swagger:export` → 0 y sin diff; 327 públicas + 32 internas = 359.
- `npm run rules:check` → 0 (90 tests, 22 suites, 52/238 IDs); `test:engine` → 0 (21);
  `test:smoke` → 0 (61).
- `TEST_DATABASE_URL=... npm run test:integration` → 0 (20 archivos / 67 tests).
- `npm run baseline:guard` → 0; `db:migrations` → 71 íntegras; `db:rls` → 46
  tablas, ninguna nueva sin RLS.
- `npm audit --audit-level=high` → 0; `--audit-level=moderate` → 13 totales
  (1 low, 12 moderate).
- **CI post-merge:** run `43088217913` sobre `develop` (`753a6ff`) en `success`
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
