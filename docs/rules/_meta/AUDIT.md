# AUDIT — Estado actual de testing vs `rules/test/`

> **Snapshot** reescrito completo en cada auditoría (R-COV-3): este archivo siempre
> describe el estado **presente** de **este repo (Core Engine)**. El histórico de
> auditorías vive en [`AUDIT-HISTORY.md`](./AUDIT-HISTORY.md); las entradas
> anteriores a 2026-10-01 pertenecen al proyecto origen y se conservan como
> procedencia.

- **Última auditoría de testing:** 2026-11-08 — **14ª de Core Engine** (R-COV-3). Revisión integral de lógica, tests, scripts y documentación sobre `feat/pack-builder` (`bb602b5`), con evidencia local ejecutada en worktree limpio.
- **Estado global:** 🔴 **NO CUMPLE — HEAD en rojo**. `npm test` falla en `identity-service` (cobertura por debajo de su umbral) y el gate de contrato `contract:breaking` falla por cuatro propiedades `parte_id` nuevas y obligatorias. G-7 (lint/build) y G-8 (OpenAPI) sí están verdes. G-4/G-5/G-6 siguen parciales y el security gate solo garantiza `high` (hay **12 moderates** de tooling).
- **Cambio de criterio:** este snapshot ya no declara verdes los gates por el hecho de estar configurados. Un gate solo se marca ✅ con la ejecución reproducible en el SHA auditado; el estado de configuración se anota aparte.

## Resumen por capa

| Capa | Estado | Evidencia |
|---|---|---|
| Unit backend | 🔴 | **1.322 tests / 132 archivos** en **13 workspaces**; 12 workspaces en verde, `identity-service` (208 tests) pasa los tests pero **rompe el gate de cobertura** (80,42/71,31/74,38 vs 89/78/85) |
| Unit frontend | N/A | No hay frontend en el repo (G-2 y R-COV-5 **reservados, nunca reutilizados**) |
| Integración (API+BD) | 🟡 | **8 archivos / 14 tests** con PostgreSQL efímero: stock, configuración de sitio, contrato de errores, tipos de orden, scheduling, migración de tenant, packs verticales e items de catálogo; faltan los flujos de orden→saga completos |
| Contrato (API) | 🔴 | Los seis shapes de error tienen prueba runtime, pero **`npm run contract:breaking -- --base-ref origin/main` sale 1**: `parte_id` se añade como propiedad obligatoria en cuatro respuestas sin allowlist en `docs/api/breaking-baseline.json` |
| E2E | 🟡 | `e2e.yml` manual/tag ejecuta `npm run demo` y verifica logs R-E-8; cubre R-E-1..7, no es gate de cada PR |
| OpenAPI (spec) | 🟢 | Export por servicio + specs versionados (**11 servicios, 334 operaciones: 310 públicas y 24 internas**); `npm run swagger:export` = exit 0 y **sin diff** (G-8 ✅) |
| Property-based | 🟡 | `fast-check` cubre dinero, ciclo de orden y parsers en `shared`; faltan fechas, límites, concurrencia y fallos de broker |
| Mutation testing | 🟢 | Stryker con `break:90` en cinco módulos críticos; mide 1–2 archivos por workspace y corre mensual/pre-release, no en PR |
| Fuzz / robustez de entradas | 🟡 | Parsers de paginación, páginas y rutas; faltan límites, outbox/saga, broker y timeouts |
| Lint (G-7) | 🟢 | ESLint: **0 errores, 45 warnings** `no-explicit-any` |
| Build (G-7) | 🟢 | `npm run build` compila shared + gateway + 11 servicios: exit 0 |
| Security gate (R-QA-6) | 🟡 | `npm audit --audit-level=high` → exit 0; `--audit-level=moderate` → exit 1 con **12 moderates** (tooling ESLint/Stryker/typescript-eslint) |
| Motor de gobernanza | 🟢 | `npm run rules:check`: **59/59 tests, 11 suites**, 41/238 IDs de regla con check ejecutable |

## Cobertura por workspace (ratchet R-COV-1, medición 2026-11-08)

| Workspace | Suites | Tests | Líneas | Ramas | Funciones | Threshold (L/B/F) | Gate |
|---|---|---|---|---|---|---|---|
| `shared` | 19 | 135 | 67,85 | 63,48 | 68,21 | 39 / 39 / 55 | ✅ |
| `api-gateway` | 7 | 116 | 85,66 | 79,06 | 72,91 | 50 / 51 / 57 | ✅ |
| `orders-service` | 8 | 91 | 86,72 | 72,87 | 80,00 | 32 / 23 / 33 | ✅ |
| `scheduling-service` | 3 | 14 | 96,55 | 83,17 | 91,11 | 94 / 77 / 88 | ✅ |
| `commissions-service` | 10 | 47 | 84,89 | 76,36 | 79,68 | 20 / 14 / 22 | ✅ |
| `finance-service` | 19 | 148 | 92,61 | 79,47 | 90,88 | 70 / 52 / 72 | ✅ |
| `market-intelligence-service` | 8 | 38 | 45,57 | 35,22 | 59,09 | 18 / 11 / 35 | ✅ |
| `catalog-service` | 13 | 166 | 96,19 | 83,80 | 97,29 | 93 / 82 / 90 | ✅ |
| `identity-service` | 19 | 208 | **80,42** | **71,31** | **74,38** | 89 / 78 / 85 | ❌ |
| `stores-service` | 10 | 168 | 95,36 | 85,64 | 98,13 | 86 / 84 / 76 | ✅ |
| `logistics-service` | 3 | 52 | 100,00 | 99,00 | 100,00 | 99 / 96 / 99 | ✅ |
| `field-service` | 2 | 23 | 85,27 | 71,69 | 73,83 | 84 / 70 / 72 | ✅ |
| `storage-service` | 11 | 116 | 100,00 | 98,85 | 100,00 | 99 / 86 / 99 | ✅ |
| **Total** | **132** | **1.322** | — | — | — | 13 workspaces | **12/13 ✅** |

- **Objetivo final (80 líneas / 70 ramas): 11 de 13 workspaces lo alcanzan**. Quedan fuera `shared` (67,85/63,48) y `market-intelligence` (45,57/35,22), que suben por ratchet (+5/release).
- **Causa del rojo de `identity-service`:** la deuda nueva de packs/partes/extensiones/capacidades (packs 66,66 líneas; partes 50,00; extensiones 73,21; capacidades 76,19) bajó el total desde ~93 a 80,42. Tres controllers nuevos no tienen spec: `capacidades.controller.ts`, `extensiones.controller.ts` y `partes.controller.ts`.
- Método: baseline medido redondeado a la baja; los thresholds viven en `scripts/config/coverage.mjs`. Ningún threshold bajó en esta auditoría.

## Métricas actuales vs mínimos

| Métrica | Valor actual | Mínimo (regla) | Cumple |
|---|---|---|---|
| Unit tests (13 workspaces) | 1.322/1.322 ejecutados; 12/13 workspaces verdes | 100% (G-1) | ❌ |
| Cobertura ≥ `coverageThreshold` | 12/13 workspaces | floor medido (R-COV-1) | ❌ |
| Cobertura objetivo final | 11/13 ≥ 80/70 | 80/70 | ⚠️ ratchet +5/release |
| Controllers con spec | **39 de 42** | R-U-10/11 (al tocar endpoints) | ❌ |
| Endpoints documentados (OpenAPI) | **334/334** en 11 servicios (310 públicas + 24 internas) | 100% (R-DO-6, R-DO-10, G-8) | ✅ |
| `*.service.ts` / `*.consumer.ts` sin spec | **2 de 54** (`shared/db/pg.service.ts`, `shared/outbox/outbox.service.ts`) | 0 (R-U-18, R-COV-2) | ❌ deuda |
| Código puro de `shared` con spec (R-U-17) | 7 de 7 | 0 sin spec | ✅ |
| Guards/pipes con spec (R-U-15) | `service-auth`, `validation.pipe` | 0 sin spec | ✅ |
| Tests integración | 14 tests en 8 archivos | ≥1 por flujo crítico API+BD (R-I-1) | ⚠️ parcial G-4 |
| Tests contrato | 6 runtime de errores + OpenAPI; `contract:breaking` rojo | ≥1 por shape (R-C-10) | ❌ |
| Tests E2E | workflow manual/release con `npm run demo` | R-E-1..7 | ⚠️ parcial G-6 |
| Property-based | 3 suites en `shared` | R-PB-1..3 | ⚠️ parcial |
| Mutación | 5 módulos con `break:90`; corrida mensual/pre-release | ≥ 90% (R-MT-2) | ✅ 5/5 |
| Lint | 0 errores / 45 warnings | 0 errores (G-7) | ✅ |
| Build | 0 errores TS | 0 errores (G-7) | ✅ |
| Audit `high` | exit 0 | 0 (R-QA-6) | ✅ |
| Audit `moderate` | **12** (tooling) | 0 (objetivo, no gate) | ⚠️ |
| Motor de reglas | 59 tests / 11 suites; 41/238 IDs | R-CI-7 | ✅ |

## Gates de CI (verificados contra `.github/workflows/ci.yml` en `bb602b5`)

| Gate | Estado | Evidencia |
|---|---|---|
| G-1 Unit backend | ❌ | `npm test` recorre 12 workspaces en verde y **aborta en `identity-service`**: tests OK, cobertura bajo umbral |
| G-2 Unit frontend | N/A | No hay frontend (ID reservado) |
| G-3 Cobertura ≥ ratchet | ❌ | `identity-service` 80,42/71,31/74,38 < 89/78/85 |
| G-4 Integración | ⏸ parcial | 8 archivos/14 tests con PostgreSQL efímero; faltan flujos de saga y outbox |
| G-5 Contrato | ❌ | `contract:breaking` sale 1 por `parte_id` obligatorio en 4 respuestas; el envelope sigue pendiente de `/api/v2` |
| G-6 E2E de flujos | ⏸ parcial release/manual | `e2e.yml` levanta el stack y ejecuta `npm run demo`; no corre en cada PR |
| G-7 Lint **y** build | ✅ | `npm run lint` 0 errores/45 warnings; `npm run build` exit 0 |
| G-8 Swagger sync | ✅ | `npm run swagger:export` exit 0 y **0 diff** (334 operaciones) |

**Total: 2 ✅ · 1 N/A · 2 ⏸ parcial · 3 ❌.**

## Deudas abiertas (orden de ataque)

1. **G-1/G-3 — `identity-service` bajo umbral**: añadir specs de controller para `capacidades`, `extensiones` y `partes`, y elevar la cobertura de `packs.service`/`partes.service`. Es el bloqueo inmediato de la suite raíz.
2. **G-5/R-C-7 — contrato roto**: decidir si `parte_id` es obligatorio en las cuatro respuestas de logística o si debe ser opcional; en el primer caso, registrar la excepción en `docs/api/breaking-baseline.json` en el mismo PR.
3. **Cobertura real de la saga**: `OutboxService` y `PgService` no tienen spec, y el event store real se sustituye por fakes. Faltan pruebas de idempotencia, versionado optimista y compensaciones.
4. **Integración estrecha**: 8 archivos; no cubren `POST /orders`, outbox→broker, comisiones, finanzas ni login real. RLS solo se valida estáticamente.
5. **`market-intelligence` (45,57/35,22) y `shared` (67,85/63,48)** siguen lejos del objetivo 80/70.
6. **3 controllers sin spec** (`identity-service`) y **2 services/consumers sin spec** (`shared`).
7. **`qa-harness` corregido en esta auditoría**: entrypoint `src/index.js`, `bcryptjs` declarado, conteo por `pass` (sin falsos verdes), lectura de profundidad de colas y README alineado. Sigue siendo **manual** (no está en CI) y su suite `15-static` duplica el pipeline.
8. **12 moderates de dependencias de tooling**; el gate R-QA-6 solo exige `high`.
9. **45 warnings `no-explicit-any`**.

## Evidencia de la auditoría (2026-11-08)

- **Worktree limpio** sobre `bb602b5`; `npm ci` exit 0.
- `npm run lint` → exit 0 (45 warnings, 0 errores).
- `npm run build` → exit 0 (shared + gateway + 11 servicios).
- `npm run swagger:export` → exit 0; `git diff` vacío; 310 operaciones públicas + 24 internas.
- `npm test` → exit 1 en `identity-service` (cobertura); 12 workspaces previos en verde.
- `npm run contract:breaking -- --base-ref origin/main` → exit 1 (4 `parte_id` obligatorios).
- `npm run rules:check` → exit 0 (59 tests, 11 suites, 41/238 IDs).
- `npm run security:secrets` → exit 0 (756 archivos); `npm audit --audit-level=high` → 0; `--audit-level=moderate` → 12.
- `npm run test:engine` → exit 0 (24 tests); `npm run test:smoke` → exit 0 (62 tests).
- `npm run test:integration` → requiere `TEST_DATABASE_URL` (en CI se provee PostgreSQL; en local sin BD no corre).

## Cambios documentales y de tooling aplicados en esta auditoría

- README raíz: 11 microservicios, `scheduling-service:3011`, 13 workspaces,
  1.322 tests, 334 operaciones OpenAPI, readiness 9/10 y variables de entorno.
- Reglas de testing: 13 workspaces, baseline de specs (2 archivos), comando
  `npm test` sin `--coverage` duplicado y estado real de G-1/G-3.
- Runbooks y `.github/README.md`: 12 imágenes (gateway + 11 servicios) y mapa de
  6 workflows.
- Plantillas stage/prod: `MINIO_PUBLIC_PORT=443` y `MINIO_PUBLIC_USE_SSL=true`
  (el público es el dominio HTTPS de Traefik, no `:9000`).
- `qa-harness`: entrypoint `.js`, `bcryptjs` declarado, sin falso verde, opciones
  CLI efectivas y README con 22 suites.
- Ver `docs/audits/2026-11-08-revision-integral.md` para la revisión de lógica,
  tests y scripts (hallazgos de seguridad y transaccionalidad).

## Cómo re-auditar

```bash
npm ci
npm run build:shared
npm test                                  # G-1/G-3: 13 workspaces con cobertura
npm run lint                              # G-7 (ESLint)
npm run build                             # G-7 (tsc)
npm run swagger:export && git diff --exit-code docs/openapi docs/api packages/api-gateway/src/openapi.agregado.ts   # G-8
npm run contract:breaking -- --base-ref origin/main   # R-C-7
npm audit --audit-level=high              # R-QA-6
npm run rules:check                       # motor de gobernanza
npm run test:engine && npm run test:smoke # autotests del motor y del smoke
TEST_DATABASE_URL=postgres://... npm run test:integration
```

Al terminar: **reescribir este snapshot** y **agregar una entrada nueva** al final de `AUDIT-HISTORY.md` (R-COV-3).
