# CHECKLIST — Revisión de PR

> Resumen ejecutable de [`rules/test/`](../test/README.md) y [`rules/git/`](../git/README.md).
> Copiar en la descripción del PR y marcar. Cada ítem remite a su regla (detalle en
> el archivo indicado por el Mapa de IDs del `README.md`). Las áreas de reglas
> doradas (`../data/`, `../db/`, `../gateway/`, `../architecture/`,
> `../microservice/`, `../observability/`) se revisan con sus propios IDs cuando el
> PR las toca.

## 1. Identificar tipo de cambio → capas obligatorias

Matriz completa en [`06-estandares-cobertura.md`](../test/06-estandares-cobertura.md). Marcar las capas ✅ que aplican:

| Tipo de cambio | Unit | Integración | Contrato | E2E |
|---|---|---|---|---|
| Función util / VO / entidad | ✅ | — | — | — |
| Use-case / lógica de dominio | ✅ | ✅ (≥1 flujo) | — | — |
| Endpoint nuevo o con filtros/paginación | ✅ | ✅ | ✅ | — |
| Endpoint con cambio de shape | ✅ | ✅ | ✅ | — |
| Pantalla/flujo de usuario nuevo | ✅ | ✅ (si consume endpoint) | — | ✅ |
| **Bug en producción** | ✅ regression | si aplica | si aplica | si es flujo visible |
| Cambio de zona horaria/fecha | ✅ boundary | ✅ día local | ✅ doc semántica | ✅ R-E-4 |

> Hoy el repo no tiene UI: la fila «Pantalla/flujo de usuario nuevo» aplica el
> día que exista (G-2 es N/A y `R-COV-5` está reservado).

## 2. Universales (todo PR)

- [ ] **R0** — Los tests asientan el requisito, no la implementación (nada de "actualizar el expected para que pase").
- [ ] **R-U-2/R-U-3** — Nombres `debe <comportamiento> cuando <condición>`; un escenario por `it`.
- [ ] **R-FL-3** — Nada depende de la timezone de la máquina (`TZ` fijada en setup).
- [ ] **R-COV-1** — La cobertura no baja respecto a `main`.
- [ ] **R-COV-2** — Todo archivo de lógica tocado (`*.service.ts`, `*.consumer.ts`, código puro de `shared`) tiene spec (o excepción documentada).
- [ ] **R-COV-5** — N/A en este repo (sin frontend; ID **reservado**, nunca se reutiliza). Si algún día hay UI, su ratchet sigue el método de R-COV-1.
- [ ] **G-7** — Lint/tsc/build en verde.

## 3. Unit ([`01-reglas-unit.md`](../test/01-reglas-unit.md))

- [ ] **R-U-4** — El test puede fallar (sin asserts triviales).
- [ ] **R-U-5/6** — Fechas con timezone explícita + boundary (cambio de día UTC vs local, fin de mes, bisiesto).
- [ ] **R-U-8/9** — Inputs `null/undefined/vacío` y parsers con input corrupto/malicioso.
- [ ] **R-U-10/11** — Controllers: además del mock, se validan los valores construidos (fechas, filtros, DTOs).
- [ ] **R-U-12/13** — Use-cases: happy path + precondición + propagación de error; errores con HTTP code + shape.
- [ ] **R-U-18** — Todo use-case nuevo trae su spec en el mismo PR.

## 4. Integración API+BD ([`02-reglas-integracion.md`](../test/02-reglas-integracion.md))

> Hoy no hay suites de integración en el repo (deuda **G-4**): la capa es
> exigible cuando exista (R-I-12); estos ítems marcan lo que debe cumplir el
> primer PR que la añada.

- [ ] **R-I-1** — Todo endpoint tocado tiene ≥1 test happy-path con BD real.
- [ ] **R-I-2** — Filtros/paginación: asertar conjunto **exacto** (conteo + IDs) y que la paginación conserva filtros.
- [ ] **R-I-3** — Filtros `dateFrom/dateTo` cruzando medianoche local, asertando contra el día local (`TZ`), no UTC.
- [ ] **R-I-4/5** — 401 sin token / 403 rol insuficiente; login completo contra BD real.
- [ ] **R-I-6/7/8** — 400 shape de error, 404/409; sin datos residuales tras fallos.
- [ ] **R-I-9/10** — Cada test crea/limpia sus datos; seed con boundary.

## 5. Contrato ([`04-reglas-contrato.md`](../test/04-reglas-contrato.md))

> Hoy no hay suites de contrato (deuda **G-5**, R-C-7): exigibles cuando existan.

- [ ] **R-C-1** — Éxito: envelope `{ data, meta: { timestamp, path, method, statusCode } }`; en listas, paginación anidada en `data` (`{ data[], total, page, limit, totalPages }`); validado por schema. Errores sin `meta` (R-C-8).
- [ ] **R-C-3** — Respuestas vía DTO; nunca entidades/rows de persistencia crudos ni campos sensibles.
- [ ] **R-C-4** — Fechas ISO 8601 UTC con `Z`.
- [ ] **R-C-5** — Semántica de `dateFrom`/`dateTo` = día local documentada.
- [ ] **R-C-8** — Errores con shape consistente (400/401/403/404/409/500).
- [ ] **R-C-11** — Campo nuevo: DTO + test de contrato + actualización de quien lo consume, en el mismo PR.

## 6. E2E ([`03-reglas-e2e.md`](../test/03-reglas-e2e.md))

> Hoy no hay E2E automatizado (deuda **G-6**): el smoke más cercano es
> `docker compose up -d --build` + `npm run demo`.

- [ ] **R-E-4** — Si toca fechas: todos los registros listados pertenecen al día local seleccionado; el día vecino NO aparece.
- [ ] **R-E-8** — Cero errores de consola durante el flujo.
- [ ] **R-E-9/10** — Verificación por datos (texto/conteos/URL), selectores estables (`data-testid`).
- [ ] **R-E-11** — Suite hermética: fixtures idempotentes creados por el test o por un seed versionado en el repo; pasa contra BD recreada de cero.

## 7. Si es bug (antes de todo lo anterior)

- [ ] **R-REG-1** — Primero el test que FALLA con el bug (rojo), luego la fix (verde); marcado `regression: <id>`.
- [ ] **R-REG-2** — El test de regresión asienta el requisito correcto (R0), no "el cambio que hicimos".
- [ ] **R-REG-3** — ¿Faltaba una capa (integración/E2E) que lo hubiera detectado antes? Si sí, agregarla.

## 8. Antes de merge (CI)

- [ ] **G-1** unit backend (job `test`, `npm test -- --coverage`) ✅
- [ ] **G-2** unit frontend — **N/A** (sin frontend; ID reservado)
- [ ] **G-3** cobertura ≥ `coverageThreshold` vigente por workspace (ratchet R-COV-1) ✅
- [ ] **G-4** integración — ⏸ sin suites (exigible cuando existan, R-I-12)
- [ ] **G-5** contrato — ⏸ sin suites (R-C-7)
- [ ] **G-7** lint + build (jobs `lint` y `build`) ✅
- [ ] **G-6** E2E de flujos — ⏸ sin job; smoke local `docker compose up -d --build` + `npm run demo`
- [ ] **G-8** spec OpenAPI — ⏸ sin script de export ni paso `git diff --exit-code` en CI (R-C-7)

## 9. Git — antes de subir ([`rules/git/`](../git/README.md))

- [ ] **R-GA-1/2/3** — Stage armado con rutas explícitas y revisado con `git diff --cached --stat`: solo archivos del cambio.
- [ ] **R-GA-4/5** — Ni artefactos ni secretos en el stage; sin `git add -f` sobre archivos ignorados.
- [ ] **R-GC-1/2** — Mensaje Conventional Commits en español e imperativo; sin `wip/fix/update`.
- [ ] **R-GC-3/4** — Un commit = una unidad temática; cita el ID de regla si aplica (`(R-ES-6)`).
- [ ] **R-GC-5** — Verificación del ámbito en verde antes del commit (G-7).
- [ ] **R-GC-7** — Sin `--amend`/rebase de commits ya publicados en `origin`.
- [ ] **R-GP-1/7** — `git log origin/main..HEAD` leído: solo commits propios y los previstos.
- [ ] **R-GP-3/4/5** — Sin force a ramas compartidas, sin WIP subido, `pull --rebase` si diverge.
- [ ] **R-GP-2/6 / R-CI-6** — Push con verificación verde y CI en verde tras el push, comprobado con `gh run list` / `gh run view` sobre el run del commit (no asumir el verde de un run anterior).

## 10. CI/CD — PRs que tocan `.github/` o definen flujos ([`rules/ci/`](../ci/README.md))

- [ ] **R-CI-1/2/3** — ¿Cambiaron gatillos, concurrencia o filtrado de eventos? Enmienda explícita de la regla, no edición silenciosa del YAML.
- [ ] **R-CI-4/5** — Pipeline único justificado; `.github/**` revisado como código (stage con rutas, sin secretos, comandos verificables en local).
- [ ] **R-EN-1/2** — Node `"20"` fijo con cache de npm y `npm ci --prefer-offline` intactos; BD de job efímera exigible cuando exista suite con BD (hoy ningún job toca BD).
- [ ] **R-EN-3/4** — Artifact de evidencia en **todos** los runs (`if: always()`); secretos únicamente en GitHub Secrets y nunca en logs.
- [ ] **R-CD-2** — N/A hoy: no existe `release.yml` (los `R-CD-*` son exigibles cuando exista, `ci/03`). Cuando exista: commit en `main` **y** run de `ci.yml` en verde en ese mismo SHA antes de publicar.
- [ ] **R-CD-7** — Ningún secreto como `ARG`/`ENV` de build ni en una capa de imagen; los secretos van en runtime.

## 11. PR — rama, cuerpo, checks y merge ([`rules/git/04-pr.md`](../git/04-pr.md))

- [ ] **R-PR-1** — El cambio vive en una rama `tipo/ámbito-descripción` creada desde `main` al día; **nada se commiteó directo a `main`**.
- [ ] **R-PR-2** — Un solo tema por PR: si el cuerpo dice "y además", se parte en dos (apiladas con `--base`, o en paralelo).
- [ ] **R-PR-3** — Verificación del ámbito en verde antes de abrir; `[WIP]` en el título si se abre en rojo, y en ese caso no se mergea.
- [ ] **R-PR-4** — Cuerpo con el checklist marcado y la tabla de verificaciones **ejecutadas** (comando + resultado), no "los tests pasan"; las capas que no aplican van justificadas.
- [ ] **R-PR-5** — Commits con Conventional Commits e ID de regla; con más de 3 commits, el cuerpo declara qué vive en cada uno.
- [ ] **R-PR-6** — `gh pr checks <n>` en verde **sobre el SHA de la cabeza**, no sobre un run anterior.
- [ ] **R-PR-7** — Merge por `--rebase`; sin `--squash` ni merge commit.
- [ ] **R-PR-8** — Tras el merge, run de `ci.yml` en `completed success` **sobre el SHA nuevo de `main`**, incluido G-6 si el PR toca un flujo de `main`.
- [ ] **R-PR-9** — Rama eliminada en local y `origin` al cerrar el PR.

> Estado actual de los gates: ver [`AUDIT.md`](./AUDIT.md) (snapshot).
