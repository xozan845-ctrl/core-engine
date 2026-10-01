# AUDIT — Estado actual de testing vs `rules/test/`

> **Snapshot** reescrito completo en cada auditoría (R-COV-3): este archivo siempre
> describe el estado **presente** de **este repo (Core Engine)**. El histórico de
> auditorías vive en [`AUDIT-HISTORY.md`](./AUDIT-HISTORY.md); las entradas
> anteriores a 2026-10-01 pertenecen al proyecto origen y se conservan como
> procedencia.

- **Última auditoría:** 2026-10-01 (primera de Core Engine — la auditoría salió de la Etapa 3 del plan de deudas; las Etapas 1–5 del plan ya están implementadas: «Cerradas en este ciclo» abajo).
- **Estado global:** 🟡 **CUMPLE PARCIAL** — gates **G-1, G-3 y G-7 efectivos y verdes en CI**, G-2 N/A y G-4/G-5/G-6/G-8 pendientes; deudas abiertas: capas de test enteras sin suites (integración, contrato, E2E, OpenAPI), robustez no implementada (property-based, mutación, fuzz), cobertura por debajo del objetivo 80/70, controllers sin specs y tooling JS fuera de ESLint.

## Resumen por capa

| Capa | Estado | Evidencia |
|---|---|---|
| Unit backend | 🟢 | **175 tests / 23 suites** en verde en **10 workspaces**; cobertura gateada por workspace con `coverageThreshold` (tabla abajo); run de `main` `36887224095` = success |
| Unit frontend | N/A | No hay frontend en el repo (G-2 y R-COV-5 **reservados, nunca reutilizados**) |
| Integración (API+BD) | 🔴 | **0 tests** — capa entera sin cubrir; deuda G-4 (R-I-* exigibles cuando la historia toque la capa) |
| Contrato (API) | 🔴 | **0 tests**; deuda G-5 (R-C-7) |
| E2E | 🔴 | **0 automatizados**; solo smoke local `docker compose up -d --build` + `npm run demo` (`scripts/smoke.mjs`); deuda G-6 |
| OpenAPI (spec) | 🟡 | `@nestjs/swagger` presente en `api-gateway`, pero **sin script de export ni paso en CI**; deuda G-8 (R-C-7) |
| Property-based | 🔴 | Sin `fast-check` en ningún paquete (R-PB-1..3) |
| Mutation testing | 🔴 | Sin Stryker; la mutación **nunca se ha ejecutado** en este repo (R-MT-1..3) |
| Fuzz / robustez de entradas | 🔴 | Sin fuzz de parsers (R-RB-1..4) |
| Lint (G-7) | 🟢 | ESLint flat, alcance `packages/*/src/**/*.ts`: **0 errores**, 47 warnings `no-explicit-any` (auditables, R-COV-3) |
| Security gate (R-QA-6) | 🟢 | `npm audit --audit-level=high` → exit 0 |
| Flujo de PR | 🟢 | **5 PRs** (#1–#5) mergeados por `--rebase`; **0 merge commits** en 15 commits; checks verdes en la cabeza del PR y run de `main` verificado tras cada merge (R-PR-1..9, R-CI-6) |

## Cobertura por workspace (ratchet R-COV-1, medido 2026-10-01)

| Workspace | Suites | Tests | Líneas | Ramas | Threshold (L/B) | Gate |
|---|---|---|---|---|---|---|
| `shared` | 1 | 8 | 9.48 | 5.18 | 9 / 5 | ✅ |
| `api-gateway` | 1 | 6 | 22.79 | 8.08 | 22 / 8 | ✅ |
| `orders-service` | 1 | 7 | 19.94 | 13.84 | 19 / 13 | ✅ |
| `commissions-service` | 1 | 5 | 12.24 | 5.08 | 12 / 5 | ✅ |
| `finance-service` | 7 | 44 | 48.25 | 41.82 | 48 / 41 | ✅ |
| `market-intelligence-service` | 2 | 7 | 15.42 | 2.20 | 15 / 2 | ✅ |
| `catalog-service` | 3 | 31 | 66.22 | 76.47 | 66 / 76 | ✅ |
| `identity-service` | 2 | 24 | 44.77 | 52.77 | 44 / 52 | ✅ |
| `stores-service` | 3 | 24 | 57.74 | 72.09 | 57 / 72 | ✅ |
| `logistics-service` | 2 | 19 | 81.11 | 92.59 | 81 / 92 | ✅ |
| **Total** | **23** | **175** | — | — | 10 workspaces | **10/10 ✅** |

- Método: baseline medido redondeado a la baja (+5 por release, nunca baja — R-COV-1). Los 6 thresholds legacy fueron recalibrados una única vez en la Etapa 2b (enmienda documentada en [`test/06`](../test/06-estandares-cobertura.md)): antes figuraban 80/80 sin evaluarse jamás.
- **Objetivo final (80 líneas / 70 ramas): 1 de 10 workspaces lo alcanza** (`logistics`); el resto sube por ratchet.

## Métricas actuales vs mínimos

| Métrica | Valor actual | Mínimo (regla) | Cumple |
|---|---|---|---|
| Unit tests (10 workspaces) | 175/175 en verde | 100% (G-1) | ✅ |
| Cobertura ≥ `coverageThreshold` | 10/10 workspaces | floor medido (R-COV-1) | ✅ gate · ⚠️ objetivo |
| Cobertura objetivo final | 1/10 ≥ 80/70 | 80/70 (tabla de umbrales) | ⚠️ ratchet +5/release |
| Controllers con spec | **0 de 25** | R-U-10/11 (al tocar endpoints) | ❌ deuda |
| `*.service.ts` / `*.consumer.ts` sin spec | **21 de 39** | 0 (R-U-18, R-COV-2) | ❌ deuda |
| Archivos `domain/` sin spec | **N/A** — 0 carpetas `domain/` en este repo | R-U-17 / R-COV-2 | N/A (estructura; IDs intactos) |
| Código puro de `shared` sin spec | **6 de 7** (solo `money.spec.ts`) | 0 (R-U-17) | ❌ deuda |
| Guards/pipes sin spec | `service-auth`, `validation.pipe` | 0 (R-U-15) | ❌ deuda |
| Tests integración | 0 | ≥1 por flujo crítico (R-I-1) | ❌ deuda G-4 |
| Tests contrato | 0 | ≥1 por shape (R-C-10) | ❌ deuda G-5 |
| Tests E2E | 0 automatizados | 7 flujos (R-E-1..7) | ❌ deuda G-6 |
| Property-based | 0 (sin `fast-check`) | R-PB-1..3 | ❌ |
| Mutación | sin ejecutar (sin Stryker) | ≥ 90% (R-MT-2) | ❌ |
| Fuzz de parsers | sin implementar | R-RB-1..4 | ❌ |
| Lint | 0 errores / 47 warns | 0 errores (G-7) | ✅ |
| Audit `high` | exit 0 | 0 (R-QA-6) | ✅ |
| Historial lineal | 0 merge commits en `main` (7 PRs `--rebase` a la fecha de esta auditoría) | 0 (R-PR-7) | ✅ |

## Gates de CI (verificados contra `.github/workflows/ci.yml`)

| Gate | Estado | Evidencia |
|---|---|---|
| G-1 Unit backend | ✅ | job `test` (`ci.yml:39`) → `npm test -- --coverage`: los 10 workspaces en verde |
| G-2 Unit frontend | N/A | No hay frontend (ID **reservado**, nunca se reutiliza) |
| G-3 Cobertura ≥ ratchet | ✅ | 10 `coverageThreshold` aplicados en cada run; el job `test` de CI genera **10 tablas de cobertura** y 0 `threshold not met` (run `36886812250`). Hallazgo cerrado en Etapa 2b: el flag `--coverage` no llegaba a `jest` (ver enmienda en `test/06`) |
| G-4 Integración | ⏸ | Sin suites de integración (exigible cuando existan, R-I-12) |
| G-5 Contrato | ⏸ | Sin suites de contrato (R-C-7) |
| G-6 E2E de flujos | ⏸ | Sin job E2E; lo más cercano hoy es el smoke local `docker compose up -d --build` + `npm run demo` |
| G-7 Lint **y** build | ✅ | job `lint` (`ci.yml:24`) → `npm run lint` 0 errores; job `build` (`ci.yml:63`) → `npm run build` 0 errores TS |
| G-8 Swagger sync | ⏸ | Sin script de export ni paso `git diff --exit-code` en CI |

**Total: 3 ✅ · 1 N/A · 4 ⏸ (deudas).**

### Controles que no son gates `G-*`

- **Security gate** (job `security-gate`, `ci.yml:10`) = regla **R-QA-6**: `npm audit --audit-level=high` encadena el resto de jobs. Exit 0 medido el 2026-10-01.

## Deudas abiertas (orden de ataque)

1. **G-4/G-5/G-6/G-8 — capas de test sin suites ni pasos en CI**: integración API+BD, contrato de API, E2E de flujos y export de OpenAPI. Son capas enteras sin cubrir, no porcentajes bajos; las reglas `R-I-*`/`R-C-*`/`R-E-*` ya son exigibles cuando una historia toque cada capa.
2. **Robustez no implementada**: property-based (R-PB-1..3, sin `fast-check`), mutation testing (R-MT-1..3, nunca ejecutada — R-MT-1 la acota a mensual/pre-release, **no debe añadirse gate de PR**) y fuzz de parsers (R-RB-1..4).
3. **Cobertura por debajo del objetivo 80/70**: solo `logistics` lo alcanza; los más bajos (`shared` 9.5, `commissions` 12.2, `market-intelligence` 15.4, `orders` 19.9, `api-gateway` 22.8 de líneas) suben por el ratchet +5/release (R-COV-1). En los 4 servicios nuevos aún no tienen specs: controllers, `seed.service` e `internal.controller`.
4. **Controllers, services y código puro sin spec**: **0/25 controllers** (R-U-10/11) y **21/39 `*.service.ts`/`*.consumer.ts`** (R-U-18, R-COV-2) — todos preexistentes al PR que los introduce; además **6 de 7 ficheros puros de `shared`** (`jwt.utils`, `order-state`, `pagination`, `errors`, `constants`, `contracts`) y guards/pipes (`service-auth`, `validation.pipe`) sin spec (R-U-17, R-U-15, revelados por la reescritura de la Etapa 5). El ratchet de cobertura es su presión.
5. **Tooling JS fuera del alcance de ESLint**: `qa-harness/`, `scripts/` y `validate-dashboards.cjs` (~160 errores acumulados) no se lintean (alcance actual `packages/*/src/**/*.ts`).
6. **R-CI-2 sin cumplir**: `ci.yml` no declara grupo de `concurrency` con `cancel-in-progress` — documentada como deuda en Etapa 2a, sin cerrar.
7. **R-FL-3 sin cumplir**: ningún setup de Jest fija `TZ`; los specs actuales no usan hora local, pero un spec futuro podría depender de la timezone de la máquina sin aviso.
8. **`tsconfig.tsbuildinfo` trackeados**: cualquier `npm run build` ensucia el árbol; higiene de repo pendiente (`chore` aparte, anotado en PR #4).
9. **47 warnings `no-explicit-any`** en `packages/*/src` — auditables con cada auditoría (R-COV-3); no son errores (G-7), pero tampoco se monitorean con un gate.

### Cerradas en este ciclo (Etapas 1–5 del plan de deudas)

- **Etapa 5: herencias del proyecto origen cerradas (deuda #8)** — `test/01..05`
  reescritos a este repo: flujos E2E de comercio (sin dashboard/asistencia/
  empleados), sin Prisma/Angular/ZK (`R-U-17` apunta ahora al código puro real
  de `shared`, `R-MT` a `identity`/`orders`/`commissions`/`finance`, `R-RB` al
  outbox/saga, `R-PB` al dinero); README raíz con recuentos reales (175 tests,
  gateway + 9 servicios, sin recomendación `pnpm` — R-EN-1); `CHECKLIST`/R-COV-2
  sin `domain/` y R-PR-8 sin Playwright. La reescritura dejó al descubierto
  deudas nuevas (shared puro y guards/pipes sin specs — ver lista de abiertas).
- **Etapa 4: npm audit de 4 moderate a 0 (R-QA-6)** — `node-cron@4`
  (`uuid@8` fuera del árbol, v4 sin dependencias) y override scopeado
  `@nestjs/swagger → js-yaml@5.4.2` (swagger 11.4.7 clava 5.3.0 exacto;
  `swagger@12` exigiría Nest 12). Gate de CI intacto: R-QA-6 solo exige high.
- **Etapa 3 (esta auditoría): deuda `_meta/` (R-COV-3)** — `AUDIT.md` reescrito a la realidad de Core Engine, `CHECKLIST.md` sin comandos ni archivos heredados (`pnpm`/`ztkeco`/`prisma`/`karma`), `AUDIT-FRONTEND.md` borrado (como preveía su propia fila en `_meta/README.md`), entrada nueva + procedencia marcadas en `AUDIT-HISTORY.md` y deuda del `_meta/README.md` cerrada.
- **Etapa 2b: G-3 no era efectivo** — `npm test -- --coverage` nunca transmitía el flag a `jest` (0 tablas de cobertura en cualquier run) y los 6 thresholds legacy 80/80 incumplían su medición (12–48 %). Cerrado: cobertura por workspace en el script raíz, 10 tablas en CI y recalibración única al baseline medido.
- **Etapa 2a: G-7 ampliado** — ESLint real (`eslint.config.mjs`, 0 errores) + job `lint` en CI.
- **Etapa 1: gates alineados con la realidad** — `ci/01..03`, `test/06` y `test/README` reescritos contra el `ci.yml` real (R-CI-1 enmendado a 4 jobs; G-1..G-8 con estado verificado).

## Evidencia

- **Local (2026-10-01, comandos de abajo):** `npm test` exit 0 → **175 tests / 23 suites** con **10 tablas de cobertura** y 0 `threshold not met` · `npm run lint` exit 0 (0 errores / 47 warnings) · `npm run build` exit 0 (0 errores TS) · `npm audit --audit-level=high` exit 0.
- **Cobertura:** medición por workspace con `npm run test -w @core/<ws> -- --coverage` (×10) y método floor (R-COV-1); revalidación con umbral aplicado: 10/10 pasan.
- **CI (R-COV-3: un gate no se marca ✅ sin run verde):** run `36886812250` sobre la cabeza del PR #5 = 4/4 jobs success (Security Gate 17s · ESLint 15s · Unit Tests + Coverage 1m18s · Build 26s), con **10 tablas de cobertura** en el log del job `test`; run `36887224095` sobre `main` = `f7282ca` → `completed success` (2m23s).
- **Flujo de PR:** `gh pr list --state merged` → 7 PRs (#1–#7) hasta esta auditoría, todos mergeados con `--rebase` y sus runs de `main` verificados (R-PR-7/8); `git rev-list --merges --count origin/main` → **0** merge commits de 18.

## Cómo re-auditar

```bash
npm test                                   # G-1/G-3: los 10 workspaces con --coverage
npm run test -w @core/<pkg> -- --coverage  # cobertura de un workspace (ratchet R-COV-1)
npm run lint                               # G-7 (ESLint, alcance packages/*/src)
npm run build                              # G-7 (tsc)
npm audit --audit-level=high               # R-QA-6 (job security-gate)
npm run demo                               # smoke de arranque (G-6 más cercano hoy)

gh run list --branch main --limit 3        # evidencia CI (R-CI-6); gh run view <id> / --log-failed
```

Al terminar: **reescribir este snapshot** y **agregar una entrada nueva** al final de `AUDIT-HISTORY.md` (R-COV-3).
