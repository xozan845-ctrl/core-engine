# AUDIT — Estado actual de testing vs `rules/test/`

> **Snapshot** reescrito completo en cada auditoría (R-COV-3): este archivo siempre
> describe el estado **presente**. El histórico de auditorías (baseline 2026-09-25,
> Fases A/B/C, post G-6/G-8) vive en [`AUDIT-HISTORY.md`](./AUDIT-HISTORY.md).

- **Última auditoría:** 2026-09-30
- **Estado global:** 🟡 **CUMPLE PARCIAL** — **8/8 gates de CI** en verde y dos controles
  de despliegue nuevos; deudas abiertas al final (fuzz de parsers, specs de dominio,
  cobertura por debajo del objetivo, RolesGuard sin usar).

## Resumen por capa

| Capa | Estado | Evidencia |
|---|---|---|
| Unit backend | 🟢 | 102 suites / **758 tests** verdes; cobertura 66.78 stmts / 66.87 branches / 66.47 funcs / 67.44 lines (alcance R-COV-4; gate ratchet 63/61/63/63) |
| Unit frontend | 🟢 | **118/118 verdes**; cobertura 53.78 / 40.86 / 45.06 / 53.42; **gate subido de 44/32/37/43 a 52/40/44/52** en este ciclo (R-COV-5) |
| Integración (API+BD) | 🟢 | 40 tests / 6 suites en `backend/test/integration/` — CI pasó `Integration tests (G-4)` |
| Contrato (API) | 🟢 | 15 tests / 1 suite en `backend/test/contract/` — CI pasó `Contract tests (G-5)` |
| Bootstrap e2e | 🟢 | 2 tests / 1 suite (`test/app.e2e-spec.ts`) — CI pasó `E2E tests (bootstrap)` |
| E2E flujos (Playwright) | 🟢 | 10/10 verdes (R-E-1..R-E-8) vía `webServer`; job `Playwright E2E de flujos (G-6)` corre **solo en push a main** |
| OpenAPI (spec) | 🟢 | `swagger:export` → `backend/docs/openapi.json` (59 paths / 42 schemas, OpenAPI 3.0.0); CI lo bloquea si está desfasado (G-8 / R-C-7) |
| Property-based | 🟢 | 18 `fc.property` + 1 test unitario en 2 specs (`R-PB-1..3`) |
| Mutation testing | 🟢 | Stryker **94.20%** ≥ break 90 en `auth`/`attendance`/`devices` (R-MT-1/2) |
| Fuzz / robustez de entradas | 🔴 | Sin fuzz de parsers (R-RB-1..4) — pendiente |
| Despliegue de esquema | 🟢 | `prisma migrate deploy` + gate de deriva (`migrate diff --exit-code`); secrets obligatorios en `docker-compose.prod.yml` |
| Flujo de PR | 🟢 | 5 PR mergeados por `--rebase`; `main` lineal con **0 merge commits** en 181 commits; checks del PR en verde antes de cada merge y run del SHA nuevo de `main` verificado tras cada uno (R-PR-1..9) |

## Métricas actuales vs mínimos

| Métrica | Valor actual | Mínimo (regla) | Cumple |
|---|---|---|---|
| Backend unit tests | 758/758 en verde | 100% | ✅ |
| Backend coverage stmts / br / fns / lines | 66.78 / 66.87 / 66.47 / 67.44 | ratchet 63/61/63/63 ✅ · objetivo 80/70/80 | ✅ gate · ⚠️ objetivo |
| Frontend coverage stmts / br / fns / lines | 53.78 / 40.86 / 45.06 / 53.42 | ratchet 52/40/44/52 ✅ · objetivo 80/70/80 | ✅ gate · ⚠️ objetivo |
| Mutación (módulos críticos) | **94.20%** (1070 killed / 65 survived / 2 timeout / 1 sin cobertura, de 1138 mutantes) | ≥ 90% (R-MT-2) | ✅ |
| Tests integración | 40 | ≥1 por flujo crítico (R-I-1) | ✅ |
| Tests contrato | 15 | ≥1 por shape (R-C-10) | ✅ |
| Tests E2E Playwright | 10 | 7 flujos (R-E-1..7) | ✅ |
| Archivos `domain/` sin spec | 99 de 124 | 0 (R-U-17) | ❌ ver deuda 2 |
| — de ellos, en las categorías que R-U-17 nombra (VO + entity + factory) | 40 (23 VO + 10 entity + 7 factory) | 0 (R-U-17) | ❌ ver deuda 2 |

## Gates de CI (verificados contra `.github/workflows/ci.yml`)

| Gate | Estado | Evidencia |
|---|---|---|
| G-1 Unit backend | ✅ | `ci.yml:47-48` paso `Unit tests (con gate de cobertura)` → `npx jest --ci --silent --coverage` |
| G-2 Unit frontend | ✅ | `ci.yml:86-87` `ng test --watch=false --code-coverage` (gate de `karma.conf.js`, R-COV-5) |
| G-3 Cobertura ≥ ratchet | ✅ | `jest.config.js` → 63/61/63/63 (R-COV-4) **y** `karma.conf.js` → 52/40/44/52 (R-COV-5) |
| G-4 Integración | ✅ | `ci.yml:151-152` paso `Integration tests (G-4)` |
| G-5 Contrato | ✅ | `ci.yml:156-157` paso `Contract tests (G-5)` |
| G-6 E2E de flujos | ✅ | job `Playwright E2E de flujos` (`ci.yml:170`) con `if: github.event_name == 'push'` (solo push a main, no PR) |
| G-7 Lint / tsc / build | ✅ | `ci.yml:42` `tsc --noEmit`, `ci.yml:45` `eslint`, `ci.yml:51`/`90` `build` (ambos workspaces) |
| G-8 Swagger sync | ✅ | `ci.yml:161` export + `ci.yml:166-167` `git diff --exit-code -- docs/openapi.json` |

**Total: 8/8 gates.**

### Controles de CI que no son gates `G-*`

Vienen de la auditoría de despliegue de 2026-09-29 (fusionada en `main` antes de este ciclo) y no tienen ID propio; se
documentan aquí para que el snapshot no se quede corto respecto a `ci.yml`:

- **Deriva de esquema** (`ci.yml:130-131` + `135-142`): `prisma migrate deploy`
  construye el schema, y un `migrate diff --from-schema-datasource
  --to-schema-datamodel --exit-code` falla si alguien edita `schema.prisma` sin
  generar migración. Sin este paso, `deploy` aplica lo que haya sin detectar el
  desfase.
- **Secretos obligatorios en producción** (`docker-compose.prod.yml`): las 9
  variables usan `${VAR:?mensaje}`, así que `docker compose config` aborta si
  falta cualquiera de ellas en vez de arrancar con un valor por defecto inseguro.

## Deudas abiertas (orden de ataque)

1. **R-RB-1..4** — Sin fuzz de parsers (query params, CSV, payload ZK). Único
   🔴 que queda: es una capa entera sin cubrir, no un porcentaje bajo.
2. **R-U-17** — 99 de 124 ficheros bajo `*/domain/` sin spec. En las categorías
   que la regla nombra explícitamente son **40** (23 value-objects, 10 entities
   y 7 factories), idéntico a la cifra de 2026-09-28; el resto son events (19),
   repositories (15), ports (3), utils (1) y 7 barrels `index.ts`. El
   denominador ha crecido de 95 a 124 desde la última auditoría.
3. **Cobertura backend 67.4% → 80%** y **frontend 53.4% → 80%** — ratchet +5 pts
   por release (R-COV-1, R-COV-5); es el mecanismo anti-regresión mientras se sube.
4. **RolesGuard** — 0 usos de `@Roles()` en controllers: hay autenticación pero
   la autorización por rol no se aplica. Sigue sin usarse desde 2026-09-27.
5. **R-MT-3** — **65 mutantes survived** (antes 17). El score sigue sobre el
   break de 90, pero ha bajado de 98.08% a **94.20%** y `attendance` es el
   responsable: 90.86%, con 52 de sus 65 supervivientes. `auth` está en 99.24%
   y `devices` en 97.03%. Dos acciones: matar los de `attendance` o
   documentarlos como equivalentes (R-MT-3), y entender por qué el score cayó
   casi 4 puntos. Ojo al mecanismo: R-MT-1 acota la mutación a periodicidad
   mensual o pre-release, así que **no** es un gate de PR por diseño y no debe
   añadirse uno. La consecuencia es que `thresholds.break` solo se comprueba
   cuando alguien ejecuta el run, y por eso la caída de 98.08% a 94.20% no la
   detectó ningún paso: salió al reejecutarlo en esta auditoría.

### Cerrada en esta auditoría

- **Ratchet del frontend sin legalizar** (deuda nº 4 del snapshot anterior). El
  gate estaba en 44/32/37/43 desde 2026-09-25 mientras la cobertura real era
  53.78/40.86/45.06/53.42: 8-10 puntos que un PR podía perder sin que CI lo
  notara. Ahora hay regla (R-COV-5) y el gate está a 52/40/44/52.
- **El flujo de PR no estaba normado.** `rules/git/` cubría `add`, `commit` y
  `push`, y su bloque de verificación daba por hecho el trabajo sobre `main`
  (`git status -sb` → `main...origin/main`). El resultado: las reglas permitían
  `commit` + `push origin main` con CI verde, **sin PR en ningún paso**, aunque
  el CHECKLIST presupusiera que existía. Ahora hay `rules/git/04-pr.md`
  (R-PR-1..9): rama por trabajo, `main` alimentado solo por merge, evidencia en
  el cuerpo, checks verdes en la cabeza del PR, merge `--rebase` y verificación
  de `main` después de mergear.

## Evidencia

- **Local (2026-09-29 y 30, con los comandos exactos de cada job):** unit backend
  102/758 con gate 63/61/63/63 exit 0 · integración 40 ✅ · contrato 15 ✅ ·
  bootstrap 2 ✅ · frontend 118/118 con gate 52/40/44/52 exit 0 ✅ ·
  `tsc --noEmit` 0 errores ✅ · `swagger:export` determinista y
  `git diff --exit-code` limpio ✅ · `pnpm install --frozen-lockfile` sin
  cambios ✅. El gate de cobertura del frontend se probó **en las dos
  direcciones**: con 52/40/44/52 sale 0, y con `branches: 99` falla con exit 1
  y `Coverage for branches (40.86%) does not meet global threshold (99%)`.
- **CI (R-COV-3: un gate no se marca ✅ sin run verde):** run `36643263353` sobre
  `main` = `completed success`, 4/4 jobs (Backend, Frontend, E2E y Playwright
  G-6). Los pasos de este snapshot apuntan a las líneas actuales de `ci.yml`
  (tabla de arriba), no a las del snapshot anterior.
- **Nota de reproducibilidad:** el `build` de backend no se puede reejecutar en
  el entorno local de esta auditoría porque `backend/dist` pertenece a `root` y
  `tsc` falla al hacer `unlink` con `EACCES`. Se verificó que es previo a
  cualquier cambio (la forma `npm run build` falla idéntico) y la compilación se
  comprobó con `tsc -p tsconfig.build.json --outDir` temporal. El build sí pasa
  en CI, que arranca de limpio.

## Cómo re-auditar

```bash
pnpm run test:backend          # unit backend
pnpm --filter zkteco-attendance-backend run test:cov   # unit + cobertura (gate R-COV-4)
cd backend && npm run test:integration   # API + BD real (requiere Postgres local)
cd backend && npm run test:contract      # contrato
cd backend && npm run test:e2e           # bootstrap + integración + contrato
cd backend && npm run swagger:export && git diff --exit-code -- docs/openapi.json   # G-8
cd backend && pnpm exec stryker run     # mutación (~6 min)

cd frontend && npx ng test --watch=false --code-coverage --browsers=ChromeHeadless  # gate R-COV-5
pnpm run test:e2e:playwright   # G-6 — webServer arranca backend+frontend si no corren
```

Al terminar: **reescribir este snapshot** y **agregar una entrada nueva** al final de `AUDIT-HISTORY.md` (R-COV-3).
