# Reglas de Testing — Core Engine

Reglas que **toda** prueba del proyecto debe cumplir. Heredadas de otro proyecto
(ZKTeco Attendance) y adoptadas aquí; el principio rector se mantiene: los tests
validaban el comportamiento erróneo de la implementación en vez del requisito
(el bug clásico: fecha filtrada como UTC en vez de día local).

## Archivos

| Archivo | Contenido |
|---|---|
| `01-reglas-unit.md` | Reglas para unit tests (utils, entidades, VO, use-cases, controllers) |
| `02-reglas-integracion.md` | Reglas para tests API + BD reales (HTTP real, BD real) |
| `03-reglas-e2e.md` | Reglas para E2E del stack completo (gateway → servicios → BD; hoy sin UI) |
| `04-reglas-contrato.md` | Reglas para contrato de API (shapes, paginación, fechas) |
| `05-reglas-robustez.md` | Reglas de calidad: property-based, mutation, edge cases, regression-first |
| `06-estandares-cobertura.md` | Umbrales de cobertura, gates de CI (con estado ✅/⏸/N/A por gate), anti-regresión |
| `07-qa-gates.md` | Reglas doradas de QA: cobertura 80 %, suites independientes, edge cases, security gate (`R-QA`) |

> Los documentos derivados de este área (**CHECKLIST, AUDIT, AUDIT-HISTORY**) no viven
> aquí: están en [`../_meta/`](../_meta/README.md). Un área de reglas solo contiene
> `README.md` + archivos `NN-tema.md`.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R0` | — | `README.md` | Principio rector |
| `R-U-*` | 1–18 | `01-reglas-unit.md` | Unit tests |
| `R-I-*` | 1–13 | `02-reglas-integracion.md` | Integración API+BD |
| `R-E-*` | 1–13 | `03-reglas-e2e.md` | E2E browser |
| `R-C-*` | 1–11 | `04-reglas-contrato.md` | Contrato de API |
| `R-REG-*` | 1–3 | `05-reglas-robustez.md` | Regression-first |
| `R-PB-*` | 1–3 | `05-reglas-robustez.md` | Property-based |
| `R-MT-*` | 1–3 | `05-reglas-robustez.md` | Mutation testing |
| `R-RB-*` | 1–4 | `05-reglas-robustez.md` | Robustez de entradas |
| `R-FL-*` | 1–4 | `05-reglas-robustez.md` | Anti-flakiness |
| `G-*` | 1–8 | `06-estandares-cobertura.md` | Gates de CI |
| `R-COV-*` | 1–5 | `06-estandares-cobertura.md` | Anti-regresión de cobertura |
| `R-QA-*` | 1–6 | `07-qa-gates.md` | Política global de QA y security gate |

> La mecánica de CI (gatillos, concurrencia, runner, evidencia) vive en
> [`../ci/`](../ci/README.md) — aquí solo los gates `G-*` y sus reglas de testing.
> Las reglas doradas de integridad que los tests protegen (centavos, stock,
> idempotencia) viven en [`../data/`](../data/README.md).

## Principio rector

> **R0 — El test valida el REQUISITO, no la implementación.**
> Si el requisito es "dateFrom=2026-09-23 significa el día local en Nicaragua",
> el test debe asertar contra esa semántica, nunca contra lo que el código
> actualmente produce. Un test que pasa con la implementación bugueada es un test inválido.

## Convenciones (mantener el "punto fijo")

1. **Formato de ID:** `R-<SUFIXO>-<n>` con `n` secuencial e incremental dentro del archivo. Un sufijo pertenece a **un solo archivo** (U, I, E, C, REG, PB, MT, RB, FL, COV, QA). Los gates de CI usan `G-<n>`. Nunca reutilizar ni renumberar IDs existentes.
2. **Formato de regla:** tabla `| ID | Regla |`; redacción en imperativo, concreta y verificable (si no se puede comprobar en un PR, no es una regla).
3. **Archivos:** numeración `NN-tema.md` con título `# NN — Tema`. Un archivo nuevo = un sufijo nuevo.
4. **Referencias cruzadas:** siempre con el ID completo (`R-U-17`), jamás "la regla 17". Antes de renombrar/eliminar una regla, `grep -rn "R-XX-n" docs/rules/` para actualizar todas las referencias (incluye `../_meta/`).
5. **Agregar una regla nueva:** añadir en su archivo → actualizar el **Mapa de IDs** de este README → reflejarla en [`../_meta/CHECKLIST.md`](../_meta/CHECKLIST.md) si afecta revisión de PR → re-auditar (R-COV-3).
6. **Estado ≠ regla:** las reglas son permanentes; el cumplimiento vive en [`../_meta/AUDIT.md`](../_meta/AUDIT.md) (snapshot) y [`../_meta/AUDIT-HISTORY.md`](../_meta/AUDIT-HISTORY.md) (log).

## Ciclo de vida

1. Toda historia/bug debe incluir sus pruebas en la capa correspondiente (§ matriz de capas en `06-estandares-cobertura.md`).
2. Todo bug nuevo: **primero** se escribe el test que falla con el bug, luego la fix (`05-reglas-robustez.md` R-REG-1).
3. La auditoría se re-ejecuta antes de cada release: se reescribe [`../_meta/AUDIT.md`](../_meta/AUDIT.md) y se agrega una entrada al final de [`../_meta/AUDIT-HISTORY.md`](../_meta/AUDIT-HISTORY.md).

## Cómo auditar (este repo)

```bash
# Suite completa con cobertura (lo que corre CI: 13 workspaces con tests).
# No añadas `-- --coverage`: el script raíz ya pasa el flag a cada workspace.
npm test

# Un solo servicio
npm run test -w @core/orders-service
npm run test -w @core/api-gateway

# Lint (job `lint`) y compilación (job `build`) — G-7
npm run lint
npm run build

# Motor de gobernanza: reglas ejecutables (job `rules-check`) y sus autotests
npm run rules:check
npm run test:engine

# Puerta de seguridad (R-QA-6 / ci.yml "Security Gate")
npm audit --audit-level=high

# E2E end-to-end sobre el stack levantado (TC-01..TC-08, RN-01..RN-08)
docker compose -f docker-compose.dev.yml up -d --build
npm run demo
```

## Pendientes (deuda abierta)

- **Tooling JS parcialmente fuera de ESLint**: `eslint.config.mjs` cubre
  `packages/*/src/**/*.ts` (el alcance de G-7) y ahora también el motor de
  gobernanza (`scripts/**/*.mjs`); `qa-harness/` sigue sin lintearse.
- **G-4 parcial**: existen suites con PostgreSQL efímero para stock y
  configuración de sitio API+BD, pero todavía faltan otros flujos críticos.
  **G-5 pendiente**: los errores 400/401 se validan en runtime para configuración
  de sitio y 403/404/409/500 en el filtro global, pero el envelope `{data,meta}` requiere una migración versionada
  `/api/v2` para no romper `/api/v1`.
  `contract:breaking` y G-8 protegen el documento OpenAPI, pero no sustituyen
  las pruebas runtime. **G-6** tiene workflow manual y de release que ejecuta `npm run demo`,
  pero no corre en cada PR; el smoke cubre los siete flujos R-E-1..7, incluido el
  boundary de fechas (R-E-4); `e2e.yml` ya inspecciona logs R-E-8. **G-8 sí está activo** en el job `build`.
- **Cobertura (ratchet R-COV-1)**: de los 13 workspaces, **12 cumplen su
  `coverageThreshold`** y **11 ya alcanzan el objetivo 80/70** de líneas/ramas.
  `identity-service` está por debajo de su umbral (89/78/85) tras añadir
  packs/partes/extensiones; `market-intelligence` y `shared` siguen bajo el
  objetivo. El detalle vigente vive en [`../_meta/AUDIT.md`](../_meta/AUDIT.md).
- **Specs pendientes (R-U-18/R-COV-2)**: el baseline de lógica sin spec se
  mantiene versionado y el recuento vigente está en
  [`../_meta/AUDIT.md`](../_meta/AUDIT.md); el `baseline:guard` impide que esa
  deuda aumente en un PR.
- **R-FL-3**: el setup central `scripts/config/test-timezone.mjs` fija
  `America/Managua` para las 13 suites Vitest y los 5 runners de mutation; CI y
  los workflows E2E/deploy también fijan la misma zona.

**Cerrado en la 3ª auditoría (2026-10-03):** `shared` puro **7/7** con spec
(R-U-17) y guards/pipes (`service-auth`, `validation.pipe`) testeados (R-U-15);
controllers de **0/25 a 24/25**. Tests 175 → 358.

Todas quedan registradas en [`../_meta/AUDIT.md`](../_meta/AUDIT.md) en la
próxima auditoría (R-COV-3).
