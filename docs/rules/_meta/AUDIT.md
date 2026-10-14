# AUDIT — Estado actual de testing vs `rules/test/`

> **Snapshot** reescrito completo en cada auditoría (R-COV-3): este archivo siempre
> describe el estado **presente** de **este repo (Core Engine)**. El histórico de
> auditorías vive en [`AUDIT-HISTORY.md`](./AUDIT-HISTORY.md); las entradas
> anteriores a 2026-10-01 pertenecen al proyecto origen y se conservan como
> procedencia.

- **Última auditoría:** 2026-10-14 — **7ª de Core Engine** (R-COV-3). Automatizó el contrato de observabilidad, cerró los huecos de scrape/correlación y añadió la regresión unitaria del header `x-request-id` en RabbitMQ.
- **Estado global:** 🟡 **CUMPLE PARCIAL** — gates **G-1, G-3, G-7 y G-8 efectivos
  y verdes en CI**, G-2 N/A y G-4/G-5/G-6 pendientes; audit de dependencias a
  **0 (high y moderate)**; deudas abiertas: capas de test enteras sin suites
  (integración, contrato, E2E), robustez no implementada
  (property-based, mutación, fuzz), cobertura por debajo del objetivo 80/70 y
  specs pendientes en services/consumers.

## Resumen por capa

| Capa | Estado | Evidencia |
|---|---|---|
| Unit backend | 🟢 | **407 tests / 61 suites** en verde en **11 workspaces**; cobertura gateada por workspace (tabla abajo); medición local 2026-10-14 |
| Unit frontend | N/A | No hay frontend en el repo (G-2 y R-COV-5 **reservados, nunca reutilizados**) |
| Integración (API+BD) | 🔴 | **0 tests** — capa entera sin cubrir; deuda G-4 (R-I-* exigibles cuando la historia toque la capa) |
| Contrato (API) | 🔴 | **0 tests**; deuda G-5 (R-C-7) |
| E2E | 🔴 | **0 automatizados**; solo smoke local `docker compose -f docker-compose.dev.yml up -d --build` + `npm run demo` (`scripts/tools/smoke.mjs`); deuda G-6 |
| OpenAPI (spec) | 🟢 | Export por servicio (`swagger:export`) + spec versionado en `docs/openapi/` (**9 servicios, 149/149 endpoints**); **documento agregado** (115 rutas / 50 schemas) servido por el gateway en `/docs`; **gate G-8** en CI (`git diff --exit-code`) |
| Property-based | 🔴 | Sin `fast-check` en ningún paquete (R-PB-1..3: dinero y ciclo de la orden) |
| Mutation testing | 🔴 | Sin Stryker; la mutación **nunca se ha ejecutado** (R-MT-1..3; módulos críticos `identity`/`orders`/`commissions`/`finance`) |
| Fuzz / robustez de entradas | 🔴 | Sin fuzz de parsers (R-RB-1..4: outbox/saga/broker) |
| Lint (G-7) | 🟢 | ESLint flat, alcance `packages/*/src/**/*.ts`: **0 errores**, 43 warnings `no-explicit-any` |
| Security gate (R-QA-6) | 🟢 | `npm audit --audit-level=high` → exit 0 **y** `--audit-level=moderate` → exit 0 (jest 30 cerró los 29 `high` de devDeps) |
| Flujo de PR | 🟢 | **118 PRs** mergeados por `--rebase`; **0 merge commits** en **76 commits** de `main`; checks verdes en la cabeza y run de la rama de entorno verificado tras cada merge (R-PR-1..9, R-GE-*) |

## Cobertura por workspace (ratchet R-COV-1, medido 2026-10-14)

| Workspace | Suites | Tests | Líneas | Ramas | Threshold (L/B) | Gate |
|---|---|---|---|---|---|---|
| `shared` | 10 | 61 | 44.69 | 41.80 | 39 / 39 | ✅ |
| `api-gateway` | 3 | 15 | 51.82 | 52.38 | 50 / 51 | ✅ |
| `orders-service` | 4 | 31 | 33.00 | 24.32 | 32 / 23 | ✅ |
| `commissions-service` | 4 | 15 | 21.38 | 15.85 | 20 / 14 | ✅ |
| `finance-service` | 13 | 77 | 71.89 | 53.04 | 70 / 52 | ✅ |
| `market-intelligence-service` | 4 | 19 | 19.93 | 12.64 | 18 / 11 | ✅ |
| `catalog-service` | 6 | 46 | 94.70 | 83.50 | 93 / 82 | ✅ |
| `identity-service` | 6 | 62 | 90.69 | 79.50 | 89 / 78 | ✅ |
| `stores-service` | 6 | 37 | 87.00 | 85.48 | 86 / 84 | ✅ |
| `logistics-service` | 3 | 24 | 100.00 | 97.14 | 99 / 96 | ✅ |
| `field-service` | 2 | 20 | 85.24 | 71.27 | 84 / 70 | ✅ |
| **Total** | **61** | **407** | — | — | 11 workspaces | **11/11 ✅** |

- **Evolución:** de 175 tests / 23 suites a **407 / 61**; la regresión de RabbitMQ comprueba que la correlación asíncrona llega al header AMQP. **Ningún threshold bajó** (R-COV-1: el ratchet solo sube).
- **Objetivo final (80 líneas / 70 ramas): 5 de 11 workspaces lo alcanzan** (`catalog`, `identity`, `stores`, `logistics`, `field`); el resto sube por ratchet (+5/release).
- Método: baseline medido redondeado a la baja; los 6 thresholds legacy se recalibraron una única vez en la Etapa 2b (enmienda en [`test/06`](../test/06-estandares-cobertura.md)).

## Métricas actuales vs mínimos

| Métrica | Valor actual | Mínimo (regla) | Cumple |
|---|---|---|---|
| Unit tests (11 workspaces) | 407/407 en verde | 100% (G-1) | ✅ |
| Cobertura ≥ `coverageThreshold` | 11/11 workspaces | floor medido (R-COV-1) | ✅ gate · ⚠️ objetivo |
| Cobertura objetivo final | 5/11 ≥ 80/70 | 80/70 | ⚠️ ratchet +5/release |
| Controllers con spec | **25 de 25** | R-U-10/11 (al tocar endpoints) | ✅ cerrado |
| Endpoints documentados (OpenAPI) | **149/149** en 9 servicios (+ agregado) | 100% (R-DO-6, G-8) | ✅ |
| `*.service.ts` / `*.consumer.ts` sin spec | **18 de 39** | 0 (R-U-18, R-COV-2) | ❌ deuda |
| Código puro de `shared` con spec (R-U-17) | **7 de 7** | 0 sin spec | ✅ cerrado |
| Guards/pipes con spec (R-U-15) | `service-auth`, `validation.pipe` | 0 sin spec | ✅ cerrado |
| Tests integración | 0 | ≥1 por flujo crítico (R-I-1) | ❌ deuda G-4 |
| Tests contrato | 0 | ≥1 por shape (R-C-10) | ❌ deuda G-5 |
| Tests E2E | 0 automatizados | 7 flujos (R-E-1..7) | ❌ deuda G-6 |
| Property-based | 0 (sin `fast-check`) | R-PB-1..3 | ❌ |
| Mutación | sin ejecutar (sin Stryker) | ≥ 90% (R-MT-2) | ❌ |
| Fuzz de parsers | sin implementar | R-RB-1..4 | ❌ |
| Lint | 0 errores / 43 warns | 0 errores (G-7) | ✅ |
| Audit `high` **y `moderate`** | exit 0 ×2 | 0 (R-QA-6) | ✅ |
| Historial lineal | 0 merge commits en `main` (118 PRs `--rebase`, 76 commits) | 0 (R-PR-7) | ✅ |

## Gates de CI (verificados contra `.github/workflows/ci.yml`)

| Gate | Estado | Evidencia |
|---|---|---|
| G-1 Unit backend | ✅ | job `test` → `npm test`: los 11 workspaces ejecutan cobertura; 407/407 verdes en la medición local |
| G-2 Unit frontend | N/A | No hay frontend (ID **reservado**, nunca se reutiliza) |
| G-3 Cobertura ≥ ratchet | ✅ | 11 `coverageThreshold` aplicados; el job `test` genera **11 tablas de cobertura** y 0 `threshold not met` |
| G-4 Integración | ⏸ | Sin suites de integración (exigible cuando existan, R-I-12) |
| G-5 Contrato | ⏸ | Sin suites de contrato (R-C-7) |
| G-6 E2E de flujos | ⏸ | Sin job E2E; lo más cercano hoy es el smoke local `docker compose -f docker-compose.dev.yml up -d --build` + `npm run demo` |
| G-7 Lint **y** build | ✅ | job `lint` → 0 errores; job `build` → 0 errores TS |
| G-8 Swagger sync | ✅ | job `build` → `npm run swagger:export` + `git diff --exit-code docs/openapi packages/api-gateway/src/openapi.agregado.ts` (149/149 endpoints, agregado); run de `main` `37505390920` (`f552627`) = success |

**Total: 4 ✅ · 1 N/A · 3 ⏸ (deudas).**

### Controles que no son gates `G-*`

- **Security gate** (job `security-gate`) = regla **R-QA-6**: `npm audit --audit-level=high`
  encadena el resto de jobs. Exit 0; tras la subida a **jest 30** (que eliminó el
  árbol con `braces`/`micromatch`) también **moderate es 0**.

## Deudas abiertas (orden de ataque)

1. **G-4/G-5/G-6 — capas de test sin suites ni pasos en CI**: integración API+BD, contrato de API y E2E de flujos. Capas enteras sin cubrir; las reglas `R-I-*`/`R-C-*`/`R-E-*` ya son exigibles cuando una historia toque cada capa. (G-8 quedó cerrado en la 5ª auditoría.)
2. **Robustez no implementada**: property-based (R-PB-1..3, sin `fast-check`), mutation testing (R-MT-1..3, nunca ejecutada — R-MT-1 la acota a mensual/pre-release, **no debe añadirse gate de PR**) y fuzz de parsers (R-RB-1..4).
3. **Cobertura por debajo del objetivo 80/70**: ya lo alcanzan 5/11 (`catalog`, `identity`, `stores`, `logistics`, `field`); los más bajos siguen siendo `market-intelligence` 19.9, `commissions` 21.4, `orders` 33.0 y `shared` 44.7 de líneas — suben por el ratchet +5/release (R-COV-1).
4. **Specs pendientes**: **18 de 39 `*.service.ts`/`*.consumer.ts`** sin spec (R-U-18, R-COV-2) — preexistentes al PR que los introduce. Controllers queda cerrado en 25/25.
5. **Tooling JS parcialmente fuera del alcance de ESLint**: `qa-harness/` sigue sin lintearse; el motor de gobernanza (`scripts/**/*.mjs`) ya se lintea (ADR-23).
6. **R-FL-3 sin cumplir**: ningún setup de Jest fija `TZ`; los specs actuales no usan hora local.
7. **43 warnings `no-explicit-any`** en `packages/*/src` — auditables con cada auditoría (R-COV-3).

### Cerradas en este ciclo (Auditoría 7ª)

- **7ª auditoría (esta, 2026-10-14):**
  - **R-OB-1/2:** checks ejecutables de logging estructurado y correlación HTTP/outbox/RabbitMQ.
  - **R-TM-1/5/6/7:** checks ejecutables de exposición y scrape universal, infraestructura/DLQ, dashboards y alertas accionables.
  - **Regresiones corregidas:** `/metrics` faltante en `market-intelligence-service`, targets 3008/3009 ausentes en Prometheus y header AMQP `x-request-id` omitido.
  - **Operación:** todas las alertas enlazan un runbook versionado; DLQ alerta por crecimiento sostenido y no por mera existencia.

### Cerradas en el ciclo anterior (Auditoría 6ª)

- **6ª auditoría (2026-10-14):**
  - **R-QA-1:** `field-service` entra en `npm test`; ya no quedan excepciones de workspace.
  - **R-U-10/11:** controller specs pasa de 24/25 a **25/25**.
  - **R-COV-1:** baseline de `field-service` medido en 85.24/71.27 y gate fijado en 84/70.
  - **Regresiones corregidas:** persistencia de `personalIds`, actualización GPS y filtros de rango de tracking.
  - **Tipado:** warnings `no-explicit-any` 47 → 43.

### Cerradas en el ciclo anterior (Auditoría 5ª)

- **5ª auditoría (2026-10-06):**
  - **G-8 (OpenAPI):** export por servicio (`swagger.ts` + `swagger-export.ts` + script `swagger:export`), specs versionados en `docs/openapi/` (**149/149 endpoints** en 9 servicios), **gate G-8** en el job `build` (`git diff --exit-code`) y **documento agregado** fusionado por `scripts/tools/swagger-aggregate.mjs` y servido por el **gateway en `/docs`** (115 rutas / 50 schemas). El agregador **aborta ante colisiones** de path/schema. Cumple **R-DO-6**.
  - **Reglas R-DO:** oficializado `docs/rules/documentation/` (R-DO-1..7).
  - **Deps (R-QA-6):** advisory `critical` nuevo en `proxy-addr` (rango `1.1.0–2.0.7`, vía `express`) → `overrides.proxy-addr = ^2.0.8`; `npm audit --audit-level=high` de nuevo **0**.

### Cerradas en ciclos anteriores (Auditoría 4ª)

- **4ª auditoría (2026-10-05):** Fixes inmediatos aplicados (PR #90):
  - **R-CI-2:** Concurrencia añadida en `ci.yml` (`cancel-in-progress`).
  - **R-OB-1:** Removido `console.log` en `market-intelligence-service` por `Logger` + manejador de error `.catch()`.
  - **R-GW-3 / R-DS-5:** Gateway limpia headers internos (`x-user-id`, `x-internal-key`, etc) de clientes antes de evaluación.
  - **R-MS-6:** Implementado `app.enableShutdownHooks()` en los 10 servicios para apagado limpio.
  - **R-AR-3 / R-MS-5:** Documentada la variable `OUTBOX_TABLA` en las plantillas `.env`.
  - **Higiene:** `*.tsbuildinfo` excluidos en `.gitignore` y eliminados del índice de git.

### Histórico (Etapas 1–5 + Fase A/B + deps/infra)

- **3ª auditoría (2026-10-03):** snapshot reescrito con evidencia fresca y
  entrada nueva en el histórico. Registra el salto de testing (175→358 tests) y
  de cobertura por la Fase A/B.
- **Fase B — specs de controllers (R-U-10/11):** de **0/25 a 24/25**; los 10
  servicios con suite (catalog 46, identity 45, orders 31, stores 37, commissions
  15, finance 77, market-intelligence 19, logistics 24 tests). `field-service`
  queda exceptuado (R-QA-1).
- **Fase A — código puro y guards/pipes (R-U-17, R-U-15):** `shared` puro **7/7**
  con spec y `service-auth`/`validation.pipe` testeados.
- **jest 29 → 30 (fix(deps), R-QA-6):** un aviso `high` nuevo (`braces`/
  `micromatch`, ~29 paquetes de devDeps) bloqueaba el gate; se subió a jest 30
  (su árbol no usa esos paquetes). Audit `high` **29 → 0**, 358 tests verdes.
- **Separación dev/prod del compose + imagen (ADR-15):** `docker-compose.yml`
  base (solo `POSTGRES_PASSWORD`) + overlay de observabilidad opcional;
  `Dockerfile` a `node:20-alpine` + `npm ci`; sin `container_name` fijo (staging
  y produccion coexisten). Plantillas `.env.example` (dev) y
  `.env.production.example` (prod).
- **Etapas 1–5 (previas):** CI real (Etapa 1), ESLint + job `lint` (2a),
  cobertura efectiva por workspace (2b), 1ª auditoría `_meta/` (3), moderates de
  npm audit a 0 (4), herencias de `test/01..05` y README reescritas (5).

## Evidencia

- **Local (2026-10-14, comandos de abajo):** `npm test` exit 0 → **407 tests / 61 suites** con **11 tablas de cobertura** y 0 `threshold not met` · `npm run lint` exit 0 (0 errores / 43 warnings) · `npm run build` exit 0 (0 errores TS) · `npm run rules:check` y `npm run test:engine` exit 0.
- **Conteos de specs (R-U-10/11/15/17/18):** 25 de 25 controllers, 21 de 39 services/consumers con spec, `shared` puro 7/7, guards/pipes con spec.
- **CI (R-COV-3: un gate no se marca ✅ sin run verde):** run `37505390920` sobre `main` (`f552627`) = 4/4 jobs success; ramas de entorno verdes en cada salto del modelo CD (R-GE-*). No se citan runs de la entrega de la propia auditoría para no crear el ciclo commit → run → commit.
- **Flujo de PR:** `gh pr list --state merged` → **118 PRs**, todos `--rebase` con run de rama de entorno verificado (R-PR-7/8, R-GE-*); `git rev-list --merges --count origin/main` → **0** merge commits de 76.

## Cómo re-auditar

```bash
npm test                                  # G-1/G-3: los 11 workspaces con cobertura
npm run test -w @core/<pkg> -- --coverage  # cobertura de un workspace (ratchet R-COV-1)
npm run lint                               # G-7 (ESLint, alcance packages/*/src)
npm run build                              # G-7 (tsc)
npm run swagger:export                     # G-8: exporta specs + agregado; debe salir 0 diff
npm audit --audit-level=high               # R-QA-6 (job security-gate)
npm audit --audit-level=moderate           # el árbol también está limpio de moderates
npm run demo                               # smoke de arranque (G-6 más cercano hoy)

# evidencia CI por rama de entorno (R-GE-1, R-CI-6); gh run view <id> / --log-failed
gh run list --branch main --limit 3
gh run list --branch staging --limit 3
gh run list --branch develop --limit 3
```

Al terminar: **reescribir este snapshot** y **agregar una entrada nueva** al final de `AUDIT-HISTORY.md` (R-COV-3).
