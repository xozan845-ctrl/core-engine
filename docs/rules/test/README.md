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
# Suite completa con cobertura (lo que corre CI: 10 workspaces con tests)
npm test -- --coverage

# Un solo servicio
npm run test -w @core/orders-service
npm run test -w @core/api-gateway

# Lint (job `lint`) y compilación (job `build`) — G-7
npm run lint
npm run build

# Puerta de seguridad (R-QA-6 / ci.yml "Security Gate")
npm audit --audit-level=high

# E2E end-to-end sobre el stack levantado (TC-01..TC-08, RN-01..RN-08)
docker compose up -d --build
npm run demo
```

## Pendientes (deuda abierta)

- **Tooling JS fuera del alcance de ESLint**: `eslint.config.mjs` cubre
  `packages/*/src/**/*.ts` (el alcance de G-7); `qa-harness/`, `scripts/` y
  `validate-dashboards.cjs` aún no se lintean (~160 errores acumulados).
- **G-4/G-5/G-6/G-8 sin paso en CI**: no existen suites de integración,
  contrato ni E2E ni paso de export OpenAPI en `ci.yml` (estado detallado en
  [`06-estandares-cobertura.md`](./06-estandares-cobertura.md)).
- **Cobertura (ratchet R-COV-1)**: los 10 workspaces cumplen su
  `coverageThreshold`, y **4 de 10 ya alcanzan el objetivo 80/70** (`catalog`
  96, `identity` 90, `stores` 91, `logistics` 100 de líneas). Los más bajos
  siguen siendo `api-gateway` 22.8, `market-intelligence` 30.3, `commissions`
  32.7 y `shared` 43.1 (suben +5 por release). `field-service` sigue sin suite
  (excepción de R-QA-1).
- **Specs pendientes (R-U-10/11, R-U-18)**: **24 de 25** `*.controller.ts` (el
  único sin spec es `field-service`, exceptuado por R-QA-1) y **21 de 39**
  `*.service.ts`/`*.consumer.ts` con spec — todos preexistentes a los PRs que
  tocan esa capa; la presión la ejerce el ratchet (recuento en
  [`../_meta/AUDIT.md`](../_meta/AUDIT.md)).
- **R-FL-3 sin cumplir**: ningún setup de Jest fija `TZ`; los specs actuales
  no usan hora local, pero un spec futuro podría depender de la timezone de la
  máquina sin que nada lo avise.

**Cerrado en la 3ª auditoría (2026-10-03):** `shared` puro **7/7** con spec
(R-U-17) y guards/pipes (`service-auth`, `validation.pipe`) testeados (R-U-15);
controllers de **0/25 a 24/25**. Tests 175 → 358.

Todas quedan registradas en [`../_meta/AUDIT.md`](../_meta/AUDIT.md) en la
próxima auditoría (R-COV-3).
