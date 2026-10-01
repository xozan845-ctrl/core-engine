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
| `03-reglas-e2e.md` | Reglas para E2E frontend → API → BD |
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
# Suite completa con cobertura (lo que corre CI: 6 workspaces con tests)
npm test -- --coverage

# Un solo servicio
npm run test -w @core/orders-service
npm run test -w @core/api-gateway

# Compilación (job `build`) — `npm run lint` hoy es no-op (sin configurar, ver Pendientes)
npm run build
npm run lint

# Puerta de seguridad (R-QA-6 / ci.yml "Security Gate")
npm audit --audit-level=high

# E2E end-to-end sobre el stack levantado (TC-01..TC-08, RN-01..RN-08)
docker compose up -d --build
npm run demo
```

## Pendientes (deuda abierta)

- **G-7 a medias**: `npm run lint` existe en la raíz pero es **no-op** — ningún
  workspace expone script `lint` y no hay ninguna config ESLint en el repo, y
  `ci.yml` no tiene paso de lint. Hasta configurarlo, G-7 se cumple solo con el
  build.
- **G-4/G-5/G-6/G-8 sin paso en CI**: no existen suites de integración,
  contrato ni E2E ni paso de export OpenAPI en `ci.yml` (estado detallado en
  [`06-estandares-cobertura.md`](./06-estandares-cobertura.md)).
- **5 workspaces sin suite**: `catalog-service`, `identity-service`,
  `stores-service`, `logistics-service` y `field-service` (R-QA-1 solo exceptúa
  `field-service`; los otros cuatro son deuda de cobertura).
- **`01`–`05` con referencias heredadas** (Prisma, frontend Angular,
  `prisma/seed.ts`): re-auditarlos contra la implementación real (R-COV-3).

Todas quedan registradas en [`../_meta/AUDIT.md`](../_meta/AUDIT.md) en la
próxima auditoría (R-COV-3).
