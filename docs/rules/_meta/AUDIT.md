# AUDIT — Estado actual de testing vs `rules/test/`

> **Snapshot** reescrito completo en cada auditoría (R-COV-3): este archivo siempre
> describe el estado **presente** de **este repo (Core Engine)**. El histórico de
> auditorías vive en [`AUDIT-HISTORY.md`](./AUDIT-HISTORY.md); las entradas
> anteriores a 2026-10-01 pertenecen al proyecto origen y se conservan como
> procedencia.

- **Última auditoría:** 2026-10-05 — **4ª de Core Engine** (R-COV-3). Resolvió deudas de observabilidad (R-OB-1), seguridad en el Gateway (R-GW-3, R-DS-5), CI (R-CI-2) y shutdown de microservicios (R-MS-6).
- **Estado global:** 🟡 **CUMPLE PARCIAL** — gates **G-1, G-3 y G-7 efectivos y
  verdes en CI**, G-2 N/A y G-4/G-5/G-6/G-8 pendientes; audit de dependencias a
  **0 (high y moderate)**; deudas abiertas: capas de test enteras sin suites
  (integración, contrato, E2E, OpenAPI), robustez no implementada
  (property-based, mutación, fuzz), cobertura por debajo del objetivo 80/70 y
  specs pendientes en services/consumers (+ `field-service`).

## Resumen por capa

| Capa | Estado | Evidencia |
|---|---|---|
| Unit backend | 🟢 | **358 tests / 55 suites** en verde en **10 workspaces**; cobertura gateada por workspace (tabla abajo); run de `main` `37135717753` (`c090ef5`) = success |
| Unit frontend | N/A | No hay frontend en el repo (G-2 y R-COV-5 **reservados, nunca reutilizados**) |
| Integración (API+BD) | 🔴 | **0 tests** — capa entera sin cubrir; deuda G-4 (R-I-* exigibles cuando la historia toque la capa) |
| Contrato (API) | 🔴 | **0 tests**; deuda G-5 (R-C-7) |
| E2E | 🔴 | **0 automatizados**; solo smoke local `docker compose up -d --build` + `npm run demo` (`scripts/smoke.mjs`); deuda G-6 |
| OpenAPI (spec) | 🟡 | `@nestjs/swagger` presente en `api-gateway`, pero **sin script de export ni paso en CI**; deuda G-8 (R-C-7) |
| Property-based | 🔴 | Sin `fast-check` en ningún paquete (R-PB-1..3: dinero y ciclo de la orden) |
| Mutation testing | 🔴 | Sin Stryker; la mutación **nunca se ha ejecutado** (R-MT-1..3; módulos críticos `identity`/`orders`/`commissions`/`finance`) |
| Fuzz / robustez de entradas | 🔴 | Sin fuzz de parsers (R-RB-1..4: outbox/saga/broker) |
| Lint (G-7) | 🟢 | ESLint flat, alcance `packages/*/src/**/*.ts`: **0 errores**, 47 warnings `no-explicit-any` |
| Security gate (R-QA-6) | 🟢 | `npm audit --audit-level=high` → exit 0 **y** `--audit-level=moderate` → exit 0 (jest 30 cerró los 29 `high` de devDeps) |
| Flujo de PR | 🟢 | **65 PRs** mergeados por `--rebase`; **0 merge commits** en **48 commits** de `main`; checks verdes en la cabeza y run de la rama de entorno verificado tras cada merge (R-PR-1..9, R-GE-*) |

## Cobertura por workspace (ratchet R-COV-1, medido 2026-10-03)

| Workspace | Suites | Tests | Líneas | Ramas | Threshold (L/B) | Gate |
|---|---|---|---|---|---|---|
| `shared` | 9 | 58 | 43.11 | 39.58 | 9 / 5 | ✅ |
| `api-gateway` | 1 | 6 | 22.79 | 12.82 | 22 / 8 | ✅ |
| `orders-service` | 4 | 31 | 44.20 | 23.16 | 19 / 13 | ✅ |
| `commissions-service` | 4 | 15 | 32.65 | 13.41 | 12 / 5 | ✅ |
| `finance-service` | 13 | 77 | 78.57 | 52.06 | 48 / 41 | ✅ |
| `market-intelligence-service` | 4 | 19 | 30.30 | 12.64 | 15 / 2 | ✅ |
| `catalog-service` | 6 | 46 | 96.00 | 83.50 | 66 / 76 | ✅ |
| `identity-service` | 5 | 45 | 90.04 | 76.69 | 44 / 52 | ✅ |
| `stores-service` | 6 | 37 | 90.84 | 85.48 | 57 / 72 | ✅ |
| `logistics-service` | 3 | 24 | 100.00 | 97.14 | 81 / 92 | ✅ |
| **Total** | **55** | **358** | — | — | 10 workspaces | **10/10 ✅** |

- **Gran salto frente a la 2ª auditoría**: de 175 tests / 23 suites a **358 / 55**, y la cobertura de cada paquete subió al añadir specs (Fase A de `shared` + Fase B de controllers). **Ningún threshold bajó** (R-COV-1: el ratchet solo sube).
- **Objetivo final (80 líneas / 70 ramas): 4 de 10 workspaces lo alcanzan** (`catalog`, `identity`, `stores`, `logistics`); el resto sube por ratchet (+5/release).
- Método: baseline medido redondeado a la baja; los 6 thresholds legacy se recalibraron una única vez en la Etapa 2b (enmienda en [`test/06`](../test/06-estandares-cobertura.md)).

## Métricas actuales vs mínimos

| Métrica | Valor actual | Mínimo (regla) | Cumple |
|---|---|---|---|
| Unit tests (10 workspaces) | 358/358 en verde | 100% (G-1) | ✅ |
| Cobertura ≥ `coverageThreshold` | 10/10 workspaces | floor medido (R-COV-1) | ✅ gate · ⚠️ objetivo |
| Cobertura objetivo final | 4/10 ≥ 80/70 | 80/70 | ⚠️ ratchet +5/release |
| Controllers con spec | **24 de 25** | R-U-10/11 (al tocar endpoints) | ⚠️ `field-service` exceptuado (R-QA-1) |
| `*.service.ts` / `*.consumer.ts` sin spec | **21 de 39** | 0 (R-U-18, R-COV-2) | ❌ deuda |
| Código puro de `shared` con spec (R-U-17) | **7 de 7** | 0 sin spec | ✅ cerrado |
| Guards/pipes con spec (R-U-15) | `service-auth`, `validation.pipe` | 0 sin spec | ✅ cerrado |
| Tests integración | 0 | ≥1 por flujo crítico (R-I-1) | ❌ deuda G-4 |
| Tests contrato | 0 | ≥1 por shape (R-C-10) | ❌ deuda G-5 |
| Tests E2E | 0 automatizados | 7 flujos (R-E-1..7) | ❌ deuda G-6 |
| Property-based | 0 (sin `fast-check`) | R-PB-1..3 | ❌ |
| Mutación | sin ejecutar (sin Stryker) | ≥ 90% (R-MT-2) | ❌ |
| Fuzz de parsers | sin implementar | R-RB-1..4 | ❌ |
| Lint | 0 errores / 47 warns | 0 errores (G-7) | ✅ |
| Audit `high` **y `moderate`** | exit 0 ×2 | 0 (R-QA-6) | ✅ |
| Historial lineal | 0 merge commits en `main` (65 PRs `--rebase`, 48 commits) | 0 (R-PR-7) | ✅ |

## Gates de CI (verificados contra `.github/workflows/ci.yml`)

| Gate | Estado | Evidencia |
|---|---|---|
| G-1 Unit backend | ✅ | job `test` → `npm test -- --coverage`: los 10 workspaces en verde (run `37135717753`) |
| G-2 Unit frontend | N/A | No hay frontend (ID **reservado**, nunca se reutiliza) |
| G-3 Cobertura ≥ ratchet | ✅ | 10 `coverageThreshold` aplicados; el job `test` genera **10 tablas de cobertura** y 0 `threshold not met` |
| G-4 Integración | ⏸ | Sin suites de integración (exigible cuando existan, R-I-12) |
| G-5 Contrato | ⏸ | Sin suites de contrato (R-C-7) |
| G-6 E2E de flujos | ⏸ | Sin job E2E; lo más cercano hoy es el smoke local `docker compose up -d --build` + `npm run demo` |
| G-7 Lint **y** build | ✅ | job `lint` → 0 errores; job `build` → 0 errores TS |
| G-8 Swagger sync | ⏸ | Sin script de export ni paso `git diff --exit-code` en CI |

**Total: 3 ✅ · 1 N/A · 4 ⏸ (deudas).**

### Controles que no son gates `G-*`

- **Security gate** (job `security-gate`) = regla **R-QA-6**: `npm audit --audit-level=high`
  encadena el resto de jobs. Exit 0; tras la subida a **jest 30** (que eliminó el
  árbol con `braces`/`micromatch`) también **moderate es 0**.

## Deudas abiertas (orden de ataque)

1. **G-4/G-5/G-6/G-8 — capas de test sin suites ni pasos en CI**: integración API+BD, contrato de API, E2E de flujos y export de OpenAPI. Capas enteras sin cubrir; las reglas `R-I-*`/`R-C-*`/`R-E-*` ya son exigibles cuando una historia toque cada capa.
2. **Robustez no implementada**: property-based (R-PB-1..3, sin `fast-check`), mutation testing (R-MT-1..3, nunca ejecutada — R-MT-1 la acota a mensual/pre-release, **no debe añadirse gate de PR**) y fuzz de parsers (R-RB-1..4).
3. **Cobertura por debajo del objetivo 80/70**: ya lo alcanzan 4/10 (`catalog`, `identity`, `stores`, `logistics`); los más bajos siguen siendo `api-gateway` 22.8, `market-intelligence` 30.3, `commissions` 32.7 y `shared` 43.1 de líneas — suben por el ratchet +5/release (R-COV-1).
4. **Specs pendientes**: **`field-service` sin suite** (excepción **R-QA-1**; es el único de los 25 controllers sin spec) y **21 de 39 `*.service.ts`/`*.consumer.ts`** sin spec (R-U-18, R-COV-2) — preexistentes al PR que los introduce.
5. **Tooling JS fuera del alcance de ESLint**: `qa-harness/`, `scripts/` y `validate-dashboards.cjs` (~160 errores acumulados) no se lintean (alcance actual `packages/*/src/**/*.ts`).
6. **R-FL-3 sin cumplir**: ningún setup de Jest fija `TZ`; los specs actuales no usan hora local.
7. **47 warnings `no-explicit-any`** en `packages/*/src` — auditables con cada auditoría (R-COV-3).

### Cerradas en este ciclo (Auditoría 4ª)

- **4ª auditoría (esta, 2026-10-05):** Fixes inmediatos aplicados (PR #90):
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

- **Local (2026-10-03, comandos de abajo):** `npm test` exit 0 → **358 tests / 55 suites** con **10 tablas de cobertura** y 0 `threshold not met` · `npm run lint` exit 0 (0 errores / 47 warnings) · `npm run build` exit 0 (0 errores TS) · `npm audit --audit-level=high` y `--audit-level=moderate` exit 0.
- **Conteos de specs (R-U-10/11/15/17/18):** 24 de 25 controllers (falta `field-service`, R-QA-1), 21 de 39 services/consumers, `shared` puro 7/7, guards/pipes con spec.
- **CI (R-COV-3: un gate no se marca ✅ sin run verde):** run `37135717753` sobre `main` (`c090ef5`) = 4/4 jobs success; ramas de entorno verdes en cada salto del modelo CD (R-GE-*). No se citan runs de la entrega de la propia auditoría para no crear el ciclo commit → run → commit.
- **Flujo de PR:** `gh pr list --state merged` → **65 PRs**, todos `--rebase` con run de rama de entorno verificado (R-PR-7/8, R-GE-*); `git rev-list --merges --count origin/main` → **0** merge commits de 48.

## Cómo re-auditar

```bash
npm test                                   # G-1/G-3: los 10 workspaces con --coverage
npm run test -w @core/<pkg> -- --coverage  # cobertura de un workspace (ratchet R-COV-1)
npm run lint                               # G-7 (ESLint, alcance packages/*/src)
npm run build                              # G-7 (tsc)
npm audit --audit-level=high               # R-QA-6 (job security-gate)
npm audit --audit-level=moderate           # el árbol también está limpio de moderates
npm run demo                               # smoke de arranque (G-6 más cercano hoy)

# evidencia CI por rama de entorno (R-GE-1, R-CI-6); gh run view <id> / --log-failed
gh run list --branch main --limit 3
gh run list --branch staging --limit 3
gh run list --branch develop --limit 3
```

Al terminar: **reescribir este snapshot** y **agregar una entrada nueva** al final de `AUDIT-HISTORY.md` (R-COV-3).