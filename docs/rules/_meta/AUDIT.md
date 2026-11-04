# AUDIT — Estado actual de testing vs `rules/test/`

> **Snapshot** reescrito completo en cada auditoría (R-COV-3): este archivo siempre
> describe el estado **presente** de **este repo (Core Engine)**. El histórico de
> auditorías vive en [`AUDIT-HISTORY.md`](./AUDIT-HISTORY.md); las entradas
> anteriores a 2026-10-01 pertenecen al proyecto origen y se conservan como
> procedencia.

- **Última auditoría de testing:** 2026-11-04 — **9ª de Core Engine** (R-COV-3). Gobernanza Fase 1 y ampliación API+BD de la Fase 2 actualizadas; la re-auditoría completa de métricas históricas queda para una fase posterior.
- **Estado global:** 🟡 **CUMPLE PARCIAL** — gates **G-1, G-3, G-7 y G-8
  configurados**, G-2 N/A, G-4 y G-6 parciales y G-5 pendiente; audit de dependencias a
  **0 (high y moderate)**; deudas abiertas: integración aún no cubre todos los
  flujos críticos, el envelope de respuestas requiere migración versionada,
  robustez parcialmente cubierta (faltan propiedades de fechas, límites,
  concurrencia y fallos; mutation requiere Node 22), cobertura por debajo del objetivo 80/70 y
  specs pendientes en services/consumers.

## Resumen por capa

| Capa | Estado | Evidencia |
|---|---|---|
| Unit backend | 🟢 | **1,105 tests / 110 archivos** en verde en **12 workspaces**; cobertura gateada por workspace (tabla abajo); CI post-merge 2026-11-04 |
| Unit frontend | N/A | No hay frontend en el repo (G-2 y R-COV-5 **reservados, nunca reutilizados**) |
| Integración (API+BD) | 🟡 | PostgreSQL efímero cubre stock y el flujo real de configuración de sitio GET/PATCH (G-4); faltan otros flujos críticos |
| Contrato (API) | 🟡 | 6 pruebas runtime cubren 400/401/403/404/409/500 y OpenAPI está versionado; el envelope R-C-1 sigue pendiente por incompatibilidad de `/api/v1` (G-5) |
| E2E | 🟡 | `e2e.yml` manual/en tags ejecuta `npm run demo` y verifica logs R-E-8; cubre los siete flujos R-E-1..7, incluido el boundary de fechas; no es gate de cada PR |
| OpenAPI (spec) | 🟢 | Export por servicio (`swagger:export`) + specs versionados en `docs/openapi/` (**11 servicios, 278/278 operaciones**); contrato público (`docs/api/core-engine.openapi.json`, 257 operaciones) e interno (`core-engine.internal.openapi.json`, 21 operaciones con `x-internal-key`); **gate G-8** en CI (`git diff --exit-code`) |
| Property-based | 🟡 | `fast-check` cubre dinero y ciclo de la orden en `shared`; faltan propiedades de fechas y flujos adicionales (R-PB-1..3) |
| Mutation testing | 🟢 | Stryker está configurado para cinco módulos críticos y el workflow usa Node 22; medición local completa: `shared` 96.95% (127/131), `identity` 97.94% (95/97), `orders` 92.37% (121/131), `commissions` 95.45% (84/88) y `finance` 95.86% (139/145); los cinco superan `break:90` (R-MT-1..3) |
| Fuzz / robustez de entradas | 🟡 | `fast-check` cubre parsers de paginación, páginas y rutas; faltan límites, outbox/saga, broker y timeouts (R-RB-1..4) |
| Lint (G-7) | 🟢 | ESLint flat, alcance `packages/*/src/**/*.ts`: **0 errores**, 45 warnings `no-explicit-any` |
| Security gate (R-QA-6) | 🟢 | `npm audit --audit-level=high` → exit 0 **y** `--audit-level=moderate` → exit 0 (jest 30 cerró los 29 `high` de devDeps) |
| Flujo de PR | 🟢 | **334 PRs** mergeados por `--rebase`; **0 merge commits** en **76 commits** de `main`; checks verdes en la cabeza y run de la rama de entorno verificado tras cada merge (R-PR-1..9, R-GE-*) |

## Cobertura por workspace (ratchet R-COV-1, medición de tests 2026-11-04)

| Workspace | Suites | Tests | Líneas | Ramas | Threshold (L/B) | Gate |
|---|---|---|---|---|---|---|
| `shared` | 17 | 125 | 62.63 | 61.93 | 39 / 39 | ✅ |
| `api-gateway` | 7 | 109 | 85.66 | 78.93 | 50 / 51 | ✅ |
| `orders-service` | 6 | 71 | 60.66 | 55.27 | 32 / 23 | ✅ |
| `commissions-service` | 10 | 47 | 85.56 | 78.30 | 20 / 14 | ✅ |
| `finance-service` | 18 | 140 | 84.38 | 70.42 | 70 / 52 | ✅ |
| `market-intelligence-service` | 5 | 21 | 26.88 | 17.61 | 18 / 11 | ✅ |
| `catalog-service` | 11 | 150 | 96.19 | 83.50 | 93 / 82 | ✅ |
| `identity-service` | 14 | 171 | 92.98 | 89.54 | 89 / 78 | ✅ |
| `stores-service` | 6 | 82 | 95.21 | 85.93 | 86 / 84 | ✅ |
| `logistics-service` | 3 | 52 | 100.00 | 100.00 | 99 / 96 | ✅ |
| `field-service` | 2 | 21 | 85.30 | 71.27 | 84 / 70 | ✅ |
| `storage-service` | 11 | 116 | 100.00 | 98.85 | 99 / 86 | ✅ |
| **Total** | **110** | **1,105** | — | — | 12 workspaces | **12/12 ✅** |

- **Evolución:** de 175 tests / 23 suites a **1,105 / 110**; métricas HTTP y correlación asíncrona tienen regresiones directas. **Ningún threshold bajó** (R-COV-1: el ratchet solo sube).
- **Objetivo final (80 líneas / 70 ramas): 9 de 12 workspaces lo alcanzan** (`api-gateway`, `catalog`, `identity`, `stores`, `logistics`, `field`, `storage`, `commissions`, `finance`); el resto sube por ratchet (+5/release).
- Método: baseline medido redondeado a la baja; los 6 thresholds legacy se recalibraron una única vez en la Etapa 2b (enmienda en [`test/06`](../test/06-estandares-cobertura.md)).

## Métricas actuales vs mínimos

| Métrica | Valor actual | Mínimo (regla) | Cumple |
|---|---|---|---|
| Unit tests (12 workspaces) | 1,105/1,105 en verde | 100% (G-1) | ✅ |
| Cobertura ≥ `coverageThreshold` | 12/12 workspaces | floor medido (R-COV-1) | ✅ gate · ⚠️ objetivo |
| Cobertura objetivo final | 9/12 ≥ 80/70 | 80/70 | ⚠️ ratchet +5/release |
| Controllers con spec | **25 de 25** | R-U-10/11 (al tocar endpoints) | ✅ cerrado |
| Endpoints documentados (OpenAPI) | **278/278** en 11 servicios (257 públicas + 21 internas; todas con respuesta documentada) | 100% (R-DO-6, R-DO-10, G-8) | ✅ |
| `*.service.ts` / `*.consumer.ts` sin spec | **13 de 39** | 0 (R-U-18, R-COV-2) | ❌ deuda |
| Código puro de `shared` con spec (R-U-17) | **7 de 7** | 0 sin spec | ✅ cerrado |
| Guards/pipes con spec (R-U-15) | `service-auth`, `validation.pipe` | 0 sin spec | ✅ cerrado |
| Tests integración | 7 tests (BD + API/BD + errores) | ≥1 por flujo crítico API+BD (R-I-1) | ⚠️ parcial G-4 |
| Tests contrato | 6 runtime de errores + OpenAPI | ≥1 por shape (R-C-10) | ⚠️ envelope pendiente G-5 |
| Tests E2E | workflow manual/release con `npm run demo` | 7 flujos (R-E-1..7) | ⚠️ 7/7 automatizados; G-6 no es gate de PR |
| Property-based | 3 suites en `shared` (`money`, `order-state`, fuzz de parsers) | R-PB-1..3 | ⚠️ parcial |
| Mutación | `shared` 96.95% (127/131), `identity` 97.94% (95/97), `orders` 92.37% (121/131), `commissions` 95.45% (84/88), `finance` 95.86% (139/145); Node 22 | ≥ 90% (R-MT-2) | ✅ 5/5 |
| Fuzz de parsers | 3 parsers cubiertos en `shared` | R-RB-1..4 | ⚠️ parcial |
| Lint | 0 errores / 43 warns | 0 errores (G-7) | ✅ |
| Audit `high` **y `moderate`** | exit 0 ×2 | 0 (R-QA-6) | ✅ |
| Historial lineal | 0 merge commits en `main` (334 PRs `--rebase`, 76 commits) | 0 (R-PR-7) | ✅ |

## Gates de CI (verificados contra `.github/workflows/ci.yml`)

| Gate | Estado | Evidencia |
|---|---|---|
| G-1 Unit backend | ✅ | job `test` → `npm test`: los 12 workspaces ejecutan cobertura; 1,105/1,105 verdes en el CI post-merge |
| G-2 Unit frontend | N/A | No hay frontend (ID **reservado**, nunca se reutiliza) |
| G-3 Cobertura ≥ ratchet | ✅ | 12 `coverageThreshold` aplicados; el job `test` genera **12 tablas de cobertura** y 0 `threshold not met` |
| G-4 Integración | ⏸ parcial | `integration-catalog` usa PostgreSQL efímero y cubre catálogo + configuración de sitio API/BD; faltan otros flujos críticos (R-I-12) |
| G-5 Contrato | ⏸ | El envelope `{data,meta}` no puede introducirse en `/api/v1` sin migración a `/api/v2`; los seis shapes de error 400/401/403/404/409/500 ya tienen prueba runtime (R-C-1/R-C-8/R-C-10) |
| G-6 E2E de flujos | ⏸ parcial release/manual | `e2e.yml` levanta el stack dev, ejecuta `npm run demo` y verifica R-E-8 en logs; cubre 7/7 flujos R-E-1..7 y no corre en cada PR |
| G-7 Lint **y** build | ✅ | job `lint` → 0 errores; job `build` → 0 errores TS |
| G-8 Swagger sync | ✅ | job `build` → `npm run swagger:export` + `git diff --exit-code docs/openapi docs/api packages/api-gateway/src/openapi.agregado.ts` (278/278 operaciones; público 257 e interno 21) |

**Total: 4 ✅ · 1 N/A · 2 ⏸ parcial · 1 ⏸ pendiente.**

### Controles que no son gates `G-*`

- **Security gate** (job `security-gate`) = regla **R-QA-6**: `npm audit --audit-level=high`
  encadena el resto de jobs. Exit 0; tras la subida a **jest 30** (que eliminó el
  árbol con `braces`/`micromatch`) también **moderate es 0**.

## Deudas abiertas (orden de ataque)

1. **G-4/G-5/G-6 — capas incompletas**: ya existe prueba API+BD para configuración de sitio, además de catálogo; faltan otros flujos críticos y la migración versionada del envelope. El E2E de release/manual cubre los siete flujos R-E-1..7 y verifica logs R-E-8. G-6 no es gate de PR. (G-8 quedó cerrado en la 5ª auditoría.)
2. **Robustez parcial**: property-based y fuzz ya cubren dinero, ciclo de orden y parsers básicos; faltan propiedades de fechas, límites, concurrencia y fallos de broker/servicios. La evidencia mutation de los cinco módulos críticos ya está completa; R-MT-1 sigue siendo mensual/pre-release y **no debe añadirse como gate de PR**.
3. **Cobertura por debajo del objetivo 80/70**: ya lo alcanzan 9/12 (`api-gateway`, `catalog`, `identity`, `stores`, `logistics`, `field`, `storage`, `commissions`, `finance`); los más bajos siguen siendo `market-intelligence` 26.88, `orders` 60.66 y `shared` 62.63 de líneas — suben por el ratchet (+5/release) (R-COV-1).
4. **Specs pendientes**: **13 de 39 `*.service.ts`/`*.consumer.ts`** sin spec (R-U-18, R-COV-2). Controllers queda cerrado en 25/25; `MetricasService`, `ReportesService`, `PedidosConsumer` de commissions y `FacturacionService` ya están cubiertos.
5. **Tooling JS parcialmente fuera del alcance de ESLint**: `qa-harness/` sigue sin lintearse; el motor de gobernanza (`scripts/**/*.mjs`) ya se lintea (ADR-23).
6. **R-FL-3 cerrado**: el setup central de Vitest/mutation y los workflows fijan `TZ=America/Managua`; la deuda de property tests de fechas R-PB-2 queda separada del E2E R-E-4, ya automatizado.
7. **45 warnings `no-explicit-any`** en `packages/*/src` — auditables con cada auditoría (R-COV-3).

### Cerradas en este ciclo (Auditoría 8ª)

- **8ª auditoría (esta, 2026-10-15):**
  - **R-TM-2/3/4:** checks ejecutables para nombres, labels, cardinalidad y métricas RED.
  - **Regresión de instrumentación:** patrón de ruta de Nest conserva `:param` en vez del ID de URL.
  - **Métricas HTTP:** namespace `core_engine_`, unidad segundos, labels `servicio/metodo/ruta/estado`, series separadas 5xx y 401/403/409.
  - **Métricas de negocio:** productores de inteligencia migrados al namespace canónico y duración de refresco en segundos.
  - **Dashboards y alertas:** migrados desde los nombres y unidad previos, con alertas separadas para fallos de servidor, credenciales y conflictos.

### Cerradas en el ciclo anterior (Auditoría 7ª)

- **7ª auditoría (2026-10-14):**
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
  - **G-8 (OpenAPI):** export por servicio (`swagger.ts` + `swagger-export.ts` + script `swagger:export`), specs versionados en `docs/openapi/` (**165/165 operaciones** en 10 servicios), **gate G-8** en el job `build` (`git diff --exit-code`) y **dos contratos agregados**: público (`docs/api/core-engine.openapi.json`, 146/146 documentadas) e interno (`core-engine.internal.openapi.json`, 19/19 con schema). El agregador separa superficies y **aborta ante colisiones** de path/schema. Cumple **R-DO-6/R-DO-10**.
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

- **CI post-merge (2026-11-04):** run `41901904723` sobre `develop` (`2dea998`) = todos los jobs success; **1,105/1,105 tests**, Rules, ESLint, Build y Catalog stock en verde.
- **Conteos de specs (R-U-10/11/15/17/18):** 25 de 25 controllers, 26 de 39 services/consumers con spec, `shared` puro 7/7, guards/pipes con spec.
- **Evidencia local focal:** `commissions-service` 47/47 tests y `finance-service` 140 tests descubiertos (2 skipped); el test de ruta HTTP de finance requiere escuchar `0.0.0.0` y queda cubierto por CI en este entorno restringido.
- **Flujo de PR:** `gh pr list --state merged` → **334 PRs**, todos `--rebase` con run de rama de entorno verificado (R-PR-7/8, R-GE-*); `git rev-list --merges --count origin/main` → **0** merge commits de 76.

## Cómo re-auditar

```bash
npm test                                  # G-1/G-3: los 12 workspaces con cobertura
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
