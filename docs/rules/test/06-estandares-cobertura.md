# 06 — Estándares de Cobertura y Gates de CI

## Matriz de capas obligatorias por tipo de cambio

Todo PR debe incluir las capas marcadas con ✅ para el tipo de cambio tocado.

| Tipo de cambio | Unit | Integración (API+BD) | Contrato | E2E |
|---|---|---|---|---|
| Función util / VO / entidad | ✅ | — | — | — |
| Use-case / lógica de dominio | ✅ | ✅ (al menos 1 flujo que lo ejercite) | — | — |
| Endpoint nuevo o con filtros/paginación | ✅ (controller) | ✅ (filtros exactos, auth, errores) | ✅ | — |
| Endpoint con cambio de shape | ✅ | ✅ | ✅ (schema actualizado) | — |
| Flujo de usuario nuevo | ✅ (componente/servicio) | ✅ (si consume endpoint) | — | ✅ |
| **Bug en producción** | ✅ regression | ✅ si es de integración/filtros | si aplica | si es flujo visible |
| Cambio de zona horaria/fecha | ✅ boundary local vs UTC | ✅ día local completo | ✅ doc de semántica | ✅ R-E-4 |

> Las columnas Integración/Contrato/E2E son exigibles cuando la historia toque
> esas capas; hoy el repo aún no tiene suites de esas capas (gates G-4, G-5 y
> G-6 figuran como pendientes abajo). La fila "Flujo de usuario nuevo" aplica
> cuando exista UI en el repo (hoy no la hay).

## Umbrales de cobertura (workspaces con suite — Vitest)

| Métrica | Mínimo global (objetivo) |
|---|---|
| Lines | ≥ 80% |
| Branches | ≥ 70% |
| Functions | ≥ 80% |

- Cobertura medida con `npm test` (que corre cada workspace con `--coverage`; es lo que ejecuta el job `test` de CI); los números se registran en `AUDIT.md` en cada auditoría (R-COV-3).
- Además de los thresholds por workspace, el job `test` ejecuta `npm run coverage:compare` contra el baseline versionado; cualquier métrica que baje falla el job (R-COV-1).
- El gate mecánico es `scripts/config/coverage.mjs` (fuente única de los porcentajes), consumido por los `thresholds` de cada `packages/*/vitest.config.ts`: **12 workspaces con suite**, todos con baseline medido redondeado a la baja (método de la línea de abajo) y subida +5 por release hasta la tabla:

  | Workspace | Líneas | Ramas | Funciones |
  |---|---|---|---|
  | `shared` | 39 | 39 | 55 |
  | `api-gateway` | 50 | 51 | 57 |
  | `orders-service` | 32 | 23 | 33 |
  | `commissions-service` | 20 | 14 | 22 |
  | `finance-service` | 70 | 52 | 72 |
  | `field-service` | 84 | 70 | 72 |
  | `catalog-service` | 93 | 82 | 90 |
  | `identity-service` | 89 | 78 | 85 |
  | `stores-service` | 86 | 84 | 76 |
  | `logistics-service` | 99 | 96 | 99 |
  | `market-intelligence-service` | 18 | 11 | 35 |
  | `storage-service` | 99 | 86 | 99 |

  **Enmienda (migración Jest → Vitest, 2026-10-14)**: el runner pasó de Jest/ts-jest a **Vitest 4** (config por workspace en `vitest.config.ts`, ver ADR-20). La cobertura se recalibró al nuevo baseline porque el proveedor de cobertura cambió y ahora instrumenta **todos** los ficheros del alcance (como hacía `collectCoverageFrom` en Jest), no solo los importados; verificado con el provider `istanbul` (mismos números). El ratchet sigue **solo sube** (nunca baja). `field-service` se incorporó con baseline 84/70/72 y ya no quedan workspaces sin suite.
- **Anti-inflado**: subir cobertura no puede lograrse agregando asserts triviales; el gate real es la mutación (R-MT-2).
- **Ratchet (gate efectivo)**: la tabla es el objetivo final; el baseline se mide con el alcance de R-COV-4 redondeado a la baja (1 pt de margen de varianza) y sube **+5 puntos por release** hasta la tabla (tope: los mínimos de la tabla). Nunca baja (R-COV-1). Cada subida queda registrada en `AUDIT.md` (R-COV-3) y se vierte subiendo el umbral en `scripts/config/coverage.mjs`. La suite `microservices-ci-contract` compara la fuente central con esta tabla y falla si un umbral queda por debajo del baseline documentado (R-COV-1).

## Umbrales de cobertura (frontend)

**N/A en este repo**: no existe `frontend/` ni suite con Karma, así que este
subapartado y `R-COV-5` quedan **reservados sin reutilizar el ID**. Si algún
día hay UI, su ratchet sigue el mismo método que el de Vitest (baseline redondeado
a la baja, +5/release, tope en la tabla, nunca baja — R-COV-1).

## Gates de CI (bloquean el merge)

Estado verificado contra `.github/workflows/ci.yml` (único workflow existente):
✅ = job existe y bloquea · ⏸ = regla vigente, paso aún no configurado (deuda en
[`README.md`](./README.md)) · N/A = no aplica a este repo (ID reservado, nunca
se reutiliza).

| Gate | Condición | Estado |
|---|---|---|
| G-1 | Suite unit de todos los workspaces con tests en verde (100 %): job `test` (`npm test -- --coverage`), encadenado tras `security-gate`. | ✅ |
| G-2 | Suite unit frontend en verde. | N/A — no hay frontend. |
| G-3 | Cobertura ≥ `thresholds` vigentes en `packages/*/vitest.config.ts` (ratchet +5/release; alcance según R-COV-4): incumplirlo o bajar el umbral falla el job `test`. | ✅ |
| G-4 | Suite de integración API+BD en verde. | ⏸ sin suites de integración (exigible cuando existan, R-I-12). |
| G-5 | Test de contrato en verde. | ⏸ sin suites de contrato (R-C-7). |
| G-6 | E2E de flujos críticos en verde antes de release (puede ser etiqueta `release`, no cada PR). | ⏸ sin job E2E; lo más cercano hoy es el smoke local `docker compose -f docker-compose.dev.yml up -d --build` + `npm run demo`. |
| G-7 | Lint **y** build sin errores: `npm run lint` (ESLint, alcance `packages/*/src/**/*.ts`, job `lint`) + `npm run build` (job `build`). | ✅ |
| G-8 | Swagger spec sincronizado (R-C-7): `git diff --exit-code` tras el export. | ✅ `npm run swagger:export` + `git diff --exit-code docs/openapi docs/api packages/api-gateway/src/openapi.agregado.ts` en el job `build`. |

> `security-gate` (job `npm audit --audit-level=high` que encadena los demás) no
> es un `G-*`: es la regla R-QA-6.

## Anti-regresión de cobertura

| ID | Regla |
|---|---|
| R-COV-1 | Ningún PR puede **bajar** la cobertura respecto al baseline versionado (`scripts/config/coverage-baseline.json`): `npm run coverage:compare` compara el reporte real de los 12 workspaces (lines/branches/functions/statements) y falla si cualquier métrica baja. Mejorar la cobertura exige actualizar el baseline con `npm run coverage:baseline` en el mismo PR; el gate nunca lo sube solo. |
| R-COV-2 | Archivos de lógica de negocio sin spec → bloquea merge (R-U-17). La suite `microservices-ci-contract` recorre `*.service.ts` y `*.consumer.ts` y falla si aparece uno sin `*.spec.ts` que no esté en `scripts/config/spec-baseline.json`; el baseline es la deuda explícita (17 archivos) y también falla si queda obsoleto al añadir el spec. |
| R-COV-3 | La auditoría `AUDIT.md` se actualiza en cada release con: cobertura actual, nº tests por capa, mutantes, deudas abiertas. Un gate solo se marca ✅ con evidencia de un **run verde de CI** (tener el paso configurado o pasar en local no basta). |
| R-COV-4 | **Alcance del gate**: solo se mide lógica ejecutable. El alcance se define en `coverage.include`/`coverage.exclude` de cada `packages/*/vitest.config.ts`: hoy `src/**/*.ts` excluyendo `main.ts` (bootstrap), `*.module.ts` (wiring DI) y `swagger*.ts` (bootstrap de OpenAPI). Exclusiones adicionales admitidas (DTOs `/dto/`, mocks `/mocks/`, scripts CLI) se añaden ahí y se documentan en `AUDIT.md`. El baseline medido con ese alcance fija los thresholds del ratchet. |
| R-COV-5 | **N/A en este repo (sin frontend) — ID reservado, no reutilizar.** Regla heredada, exigible si algún día hay UI: el ratchet del frontend es norma, no una nota en el config — sigue el mismo método que el de Jest (baseline medido, redondeado a la baja con 1 pt de margen, +5 pts por release, tope en la tabla de umbrales y **nunca baja**, R-COV-1). Sin esta regla, G-3 protege solo los workspaces backend: la UI puede perder cobertura hasta el nivel del gate sin que nada lo note, porque un threshold obsoleto nunca falla. |
