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
> esas capas; G-4 cubre la matriz multi-vertical, packs, RLS, enforcement, piloto y Outbox
> sobre PostgreSQL efímero; G-5 queda cubierto por el envelope `{data,meta}` de
> `/api/v2` y G-6 es gate de release. La fila "Flujo de usuario nuevo" aplica
> cuando exista UI en el repo (hoy no la hay).

## Cobertura de los servicios Rust (migraciones ADR-0004, ADR-0006, ADR-0007, ADR-0008, ADR-0009, ADR-0010, ADR-0012 y ADR-0013)

`packages/scheduling-service`, `packages/notifications-service`,
`packages/commissions-service`, `packages/field-service`,
`packages/market-intelligence-service`, `packages/logistics-service`,
`packages/storage-service` y `packages/orders-service` se miden con
`cargo-llvm-cov` en el job `rust` de CI
(matriz por servicio), con
PostgreSQL/PostGIS efímero para ejecutar también las pruebas de integración:

```bash
# scheduling-service
cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' \
  --fail-under-lines 71 --fail-under-functions 65

# notifications-service
cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' \
  --fail-under-lines 74 --fail-under-functions 74

# commissions-service
cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' \
  --fail-under-lines 72 --fail-under-functions 68

# field-service
cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' \
  --fail-under-lines 77 --fail-under-functions 71

# logistics-service
cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' \
  --fail-under-lines 63 --fail-under-functions 62

# market-intelligence-service
cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' \
  --fail-under-lines 63 --fail-under-functions 60

# storage-service
cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' \
  --fail-under-lines 54 --fail-under-functions 52

# orders-service
cargo llvm-cov --locked --ignore-filename-regex 'src/main\.rs' \
  --fail-under-lines 62 --fail-under-functions 63
```

- **Alcance** (equivalente a R-COV-4): todo `src/**`, excluyendo `main.rs`
  (bootstrap). `llvm-cov` no mide ramas: la métrica de branches queda **N/A**
  para estos servicios y se justifica aquí (no se inventa un número).
- **Baseline medido**: `scheduling-service` 72,86 % líneas y 66,82 %
  funciones (2026-11-12); `notifications-service` 75,36 % líneas y 75,38 %
  funciones (2026-11-12); `commissions-service` 73,29 % líneas y 69,70 %
  funciones con PostgreSQL efímero; `field-service` 78,52 % líneas y 72,38 %
  funciones (2026-11-13, suites PostgreSQL y HTTP); `logistics-service`
  64,57 % líneas y 63,42 % funciones; `market-intelligence-service`
  64,41 % líneas y 61,82 % funciones con PostgreSQL/PostGIS (2026-11-13);
  `orders-service` 62,70 % líneas y 64,35 % funciones con PostgreSQL
  (2026-11-13); `storage-service` con gate 54/52 sobre su suite Rust.
  Los gates arrancan en 71/65, 74/74, 72/68, 77/71, 63/62, 63/60, 54/52 y 62/63 (redondeo a
  la baja con el margen de 1 punto) y suben +5 por release hasta el objetivo
  histórico 94/77/88 de R-COV-1.
- Mientras dure cada migración conviven el gate Vitest del TS (retirado en el
  PR de conmutación) y este gate Rust; los ADR fijan el orden.
- `rabbit.rs` y, en `notifications-service`, `consumidor.rs`/`db.rs`, y en
  `commissions-service` `outbox.rs`/el worker tienen baja cobertura unitaria
  porque requieren broker, base de datos o tiempo real; en `field-service` los
  handlers HTTP dependen de PostgreSQL (`tests/http_field.rs`), `logistics`
  cubre guías (`tests/envios_pg.rs`) y `market-intelligence` analítica
  (`tests/`). La integración se valida en el job `rust` y en el
  smoke de `staging` (R-PR-8). Subir esa cobertura es deuda registrada para la
  siguiente auditoría (R-COV-3).

## Umbrales de cobertura (workspaces con suite — Vitest)

| Métrica | Mínimo global (objetivo) |
|---|---|
| Lines | ≥ 80% |
| Branches | ≥ 70% |
| Functions | ≥ 80% |

- Cobertura medida con `npm test` (que corre cada workspace con `--coverage`; es lo que ejecuta el job `test` de CI); los números se registran en `AUDIT.md` en cada auditoría (R-COV-3).
- Además de los thresholds por workspace, el job `test` ejecuta `npm run coverage:compare` contra el baseline versionado; cualquier métrica que baje falla el job (R-COV-1).
- El gate mecánico es `scripts/config/coverage.mjs` (fuente única de los porcentajes), consumido por los `thresholds` de cada `packages/*/vitest.config.ts`: **8 workspaces con suite**, todos con baseline medido redondeado a la baja (método de la línea de abajo) y subida +5 por release hasta la tabla:

  | Workspace | Líneas | Ramas | Funciones |
  |---|---|---|---|
  | `shared` | 39 | 39 | 55 |
  | `api-gateway` | 50 | 51 | 57 |
  | `finance-service` | 70 | 52 | 72 |
  | `catalog-service` | 93 | 82 | 90 |
  | `identity-service` | 89 | 78 | 85 |
  | `stores-service` | 86 | 84 | 76 |

  **Enmienda (migración Jest → Vitest, 2026-10-14)**: el runner pasó de Jest/ts-jest a **Vitest 4** (config por workspace en `vitest.config.ts`, ver ADR-20). La cobertura se recalibró al nuevo baseline porque el proveedor de cobertura cambió y ahora instrumenta **todos** los ficheros del alcance (como hacía `collectCoverageFrom` en Jest), no solo los importados; verificado con el provider `istanbul` (mismos números). El ratchet sigue **solo sube** (nunca baja). `scheduling-service` salió del runner Vitest al migrar a Rust (ADR-0004), `notifications-service` (ADR-0006), `commissions-service` (ADR-0007), `field-service` (ADR-0008), `market-intelligence-service` (ADR-0009), `logistics-service` (ADR-0010) y `orders-service` (ADR-0013): su gate es el de la sección anterior (`cargo-llvm-cov`), no un workspace npm.
- **Anti-inflado**: subir cobertura no puede lograrse agregando asserts triviales; el gate real es la mutación (R-MT-2).
- **Ratchet (gate efectivo)**: la tabla es el objetivo final; el baseline se mide con el alcance de R-COV-4 redondeado a la baja (1 pt de margen de varianza) y sube **+5 puntos por release** hasta la tabla (tope: los mínimos de la tabla). Nunca baja (R-COV-1). Cada subida queda registrada en `AUDIT.md` (R-COV-3) y se vierte subiendo el umbral en `scripts/config/coverage.mjs`. La suite `microservices-ci-contract` compara la fuente central con esta tabla y falla si un umbral queda por debajo del baseline documentado (R-COV-1).

## Umbrales de cobertura (frontend)

**N/A en este repo**: no existe `frontend/` ni suite con Karma, así que este
subapartado y `R-COV-5` quedan **reservados sin reutilizar el ID**. Si algún
día hay UI, su ratchet sigue el mismo método que el de Vitest (baseline redondeado
a la baja, +5/release, tope en la tabla, nunca baja — R-COV-1).

## Gates de CI (bloquean el merge)

Estado verificado contra `.github/workflows/ci.yml` (uno de los seis workflows:
`ci`, `deploy`, `e2e`, `mutation`, `promote`, `release`):
✅ = job existe y bloquea · ⏸ = regla vigente, paso aún no configurado (deuda en
[`README.md`](./README.md)) · N/A = no aplica a este repo (ID reservado, nunca
se reutiliza).

| Gate | Condición | Estado |
|---|---|---|
| G-1 | Suite unit de todos los workspaces con tests en verde (100 %): job `test` (`npm test`), encadenado tras `security-gate`. | ✅ configurado · estado en [`AUDIT.md`](../_meta/AUDIT.md) |
| G-2 | Suite unit frontend en verde. | N/A — no hay frontend. |
| G-3 | Cobertura ≥ `thresholds` vigentes en `packages/*/vitest.config.ts` (ratchet +5/release; alcance según R-COV-4): incumplirlo o bajar el umbral falla el job `test`. | ✅ configurado · estado en [`AUDIT.md`](../_meta/AUDIT.md) |
| G-4 | Suite de integración API+BD en verde. | ✅ PostgreSQL efímero cubre matriz multi-vertical, packs v1/v2, RLS, enforcement, pilotos y concurrencia de Outbox (59 tests). |
| G-5 | Test de contrato en verde. | ✅ envelope `{data,meta}` en todo `/api/v2`; errores 400/401/403/404/409/500 verificados en runtime; R-C-1 comprobado sobre el OpenAPI público. |
| G-6 | E2E de flujos críticos en verde antes de release (puede ser etiqueta `release`, no cada PR). | ✅ **gate de release**: `release.yml` llama a `e2e.yml` antes de `images`; el job levanta `docker-compose.dev.yml`, ejecuta `npm run demo` y verifica logs R-E-8 (R-E-1…R-E-7, incluido R-E-4). Un fallo bloquea la publicación. `deploy.yml` además ejecuta el smoke remoto de stage/prod (R-CD-12). |
| G-7 | Lint **y** build sin errores: `npm run lint` (ESLint, alcance `packages/*/src/**/*.ts`, job `lint`) + `npm run build` (job `build`). | ✅ |
| G-8 | Swagger spec sincronizado (R-C-7): `git diff --exit-code` tras el export. | ✅ `npm run swagger:export` + `git diff --exit-code docs/openapi docs/api packages/api-gateway/src/openapi.agregado.ts` en el job `build`. |

> `security-gate` (job `npm audit --audit-level=high` que encadena los demás) no
> es un `G-*`: es la regla R-QA-6.

## Anti-regresión de cobertura

| ID | Regla |
|---|---|
| R-COV-1 | Ningún PR puede **bajar** la cobertura respecto al baseline versionado (`scripts/config/coverage-baseline.json`): `npm run coverage:compare` compara el reporte real de los 8 workspaces Vitest (lines/branches/functions/statements) y, en paralelo, el job `rust` aplica el gate de `cargo-llvm-cov` y falla si cualquier métrica baja. Mejorar la cobertura exige actualizar el baseline con `npm run coverage:baseline` en el mismo PR; el gate nunca lo sube solo. |
| R-COV-2 | Archivos de lógica de negocio sin spec → bloquea merge (R-U-17). La suite `microservices-ci-contract` recorre `*.service.ts` y `*.consumer.ts` y falla si aparece uno sin `*.spec.ts` que no esté en `scripts/config/spec-baseline.json`; el baseline está vacío: **0 servicios/consumers sin spec**; también falla si queda obsoleto al añadir el spec. |
| R-COV-3 | La auditoría `AUDIT.md` se actualiza en cada release con: cobertura actual, nº tests por capa, mutantes, deudas abiertas. Un gate solo se marca ✅ con evidencia de un **run verde de CI** (tener el paso configurado o pasar en local no basta). |
| R-COV-4 | **Alcance del gate**: solo se mide lógica ejecutable. El alcance se define en `coverage.include`/`coverage.exclude` de cada `packages/*/vitest.config.ts`: hoy `src/**/*.ts` excluyendo `main.ts` (bootstrap), `*.module.ts` (wiring DI) y `swagger*.ts` (bootstrap de OpenAPI). Exclusiones adicionales admitidas (DTOs `/dto/`, mocks `/mocks/`, scripts CLI) se añaden ahí y se documentan en `AUDIT.md`. El baseline medido con ese alcance fija los thresholds del ratchet. |
| R-COV-5 | **N/A en este repo (sin frontend) — ID reservado, no reutilizar.** Regla heredada, exigible si algún día hay UI: el ratchet del frontend es norma, no una nota en el config — sigue el mismo método que el de Jest (baseline medido, redondeado a la baja con 1 pt de margen, +5 pts por release, tope en la tabla de umbrales y **nunca baja**, R-COV-1). Sin esta regla, G-3 protege solo los workspaces backend: la UI puede perder cobertura hasta el nivel del gate sin que nada lo note, porque un threshold obsoleto nunca falla. |
