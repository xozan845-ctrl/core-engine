# scripts/ — Motor de gobernanza

Este directorio es el **mecanismo que mantiene el proyecto en su punto fijo**:
ejecuta de forma automática las reglas de [`docs/rules/`](../docs/rules/README.md)
que son verificables por máquina, y aloja las operaciones que no son gobernanza
(build de contrato, demo E2E).

- **Reglas ejecutables:** job `rules-check` de CI (`.github/workflows/ci.yml`) y
  `npm run rules:check`.
- **Ancla normativa:** **R-CI-7** (existen todas las suites del manifiesto) y el
  área [`ci/`](../docs/rules/ci/README.md).
- **Decisión de arquitectura:** [ADR-23](../docs/decisiones.md).

> Principio rector: el motor de reglas **se aplica a sí mismo** los estándares
> del proyecto (tests propios, lint, contrato explícito). Un validador que no se
> valida es una promesa, no una garantía.

## Arquitectura

Cinco capas con dependencias en **un solo sentido**.

```
scripts/
├── README.md                 # este documento: contrato y mapa del motor
├── cli.mjs                   # ÚNICO entrypoint (composition root)
├── check-rules.mjs           # wrapper de compatibilidad -> cli.mjs rules
│
├── engine/                   # infra reutilizable (CERO conocimiento del proyecto)
│   ├── runner.mjs            #   descubrimiento + manifiesto + ejecución
│   ├── suite.mjs             #   contrato defineSuite(...)
│   ├── report.mjs            #   plan de suites + cobertura de reglas
│   └── lib/
│       ├── paths.mjs         #   repoRoot, resolveInRepo(), toFileUrl()
│       ├── fs.mjs            #   walkFiles, walkMarkdown, readText, readJson
│       ├── git.mjs           #   wrappers seguros (execFileSync, sin interpolación)
│       ├── commits.mjs       #   validateCommitSubject (pura, parametrizada)
│       ├── semver.mjs        #   parse/compare de tags vX.Y.Z[-pre] (puro)
│       ├── coverage.mjs      #   lee coverage-summary y compara contra baseline
│       └── rules-docs.mjs    #   áreas, archivos NN-tema.md e IDs de docs/rules
│
├── config/                   # datos y helpers puros del proyecto
│   ├── project.mjs           #   rama, servicios, scopes, rutas, tipos de commit
│   ├── coverage.mjs          #   umbrales de cobertura por workspace (R-COV-1/4)
│   ├── coverage-baseline.json #  cobertura real de referencia (R-COV-1)
│   ├── spec-baseline.json    #   services/consumers sin spec (R-COV-2)
│   └── manifest.mjs          #   suites exigidas (id + archivo)
│
├── rules/                    # reglas ejecutables, agrupadas por dominio
│   ├── docs/structure.test.mjs     # integridad de docs/rules + manifiesto
│   ├── git/commits.test.mjs        # R-GC-1..4 (puro)
│   ├── git/branch.test.mjs         # R-GC-1..4 (contextual, commits del PR)
│   ├── delivery/compose.test.mjs   # R-CD-1/3/7/8
│   ├── microservices/
│   │   ├── structure.test.mjs      # R-CD-3/4/14
│   │   └── ci-contract.test.mjs    # R-MS-1, R-QA-1, R-COV-2/4, R-CI-5
│   ├── contract/
│   │   └── openapi.test.mjs        # R-DO-8, R-DS-5: superficies y contrato OpenAPI
│   └── observability/
│       ├── tracing.test.mjs        # R-OB-1/2
│       └── telemetry.test.mjs      # R-TM-1..7
│
└── tools/                    # operaciones (NO gobernanza)
    ├── release-check.mjs      # valida tag, monotonicidad y versiones (R-CD-1/2)
    ├── release-check.test.mjs # fixtures git: tag anotado, orden y versiones
    ├── version-bump.mjs       # alinea package.json + lockfile con un release
    ├── version-bump.test.mjs  # bump de root/workspaces/lock y dry-run
    ├── coverage-compare.mjs   # gate de cobertura real vs baseline (R-COV-1)
    ├── coverage-baseline.mjs  # actualiza el baseline desde coverage-summary
    ├── coverage-compare.test.mjs # gate y writer con reportes sintéticos
    ├── smoke.mjs              # flujo API completo contra stack levantado
    ├── smoke.test.mjs         # regresiones del runner con HTTP efímero
    ├── swagger-aggregate.mjs  # agrega specs y separa superficie pública/interna
    └── swagger-aggregate.test.mjs # regresiones del agregador (filtro y colisiones)
```

### Regla de dependencia

| Capa | Puede importar de | No puede importar de |
|---|---|---|
| `engine/` | Node built-ins | `config/`, `rules/`, `tools/` |
| `config/` | — (sólo datos) | `engine/`, `rules/` |
| `rules/` | `engine/`, `config/` | `tools/` |
| `tools/` | `engine/lib/`, `config/` | `rules/` |
| `cli.mjs` | todo (es el composition root) | — |

Esto es lo que hace **portable** el motor: `engine/` + `config/` se copian a otro
repo del stack sin arrastrar una sola regla.

## Contrato de una suite

Cada suite se declara con `defineSuite` y registra sus `test()` de `node:test`.
El runner la descubre **importándola con el registro desactivado**: lee la
metadata sin ejecutar los tests, y luego lanza `node --test` en un proceso
nuevo. El aislamiento es obligatorio: reutilizar el proceso de descubrimiento
haría que la caché ESM impidiera registrar las assertions y produciría un falso
verde.

```js
// scripts/rules/delivery/compose.test.mjs
import { defineSuite } from '../../engine/suite.mjs';
import { repoRoot } from '../../engine/lib/paths.mjs';
import { readText } from '../../engine/lib/fs.mjs';

export default defineSuite({
  id: 'delivery-compose',
  area: 'cd',                              // área de docs/rules que cubre
  rules: ['R-CD-1', 'R-CD-3', 'R-CD-7', 'R-CD-8'],
  mode: 'repo',                            // 'pure' (sin disco/git) | 'repo' (inspecciona el repo)
  run: ({ test, assert }) => {
    test('R-CD-3: los composes de stage/prod consumen GHCR', () => {
      // …
    });
  },
});
```

| Campo | Para qué sirve |
|---|---|
| `id` | Identificador único de la suite (reporte y filtros). |
| `area` | Área de `docs/rules/` que la suite vigila. |
| `rules` | IDs cubiertos → alimenta el **reporte de cobertura**. |
| `mode` | `pure` corre sin git/red (rápido, local); `repo` inspecciona el repo real. |
| `run` | Registra los `test(...)`; recibe `{ test, assert }` inyectados. |

## Manifiesto (`config/manifest.mjs`)

Sucede a `REQUIRED_RULE_SUITES` conservando **R-CI-7**: si falta una suite
exigida, `rules:check` falla. Cada entrada liga el `id` de la suite con su
archivo.

```js
export const requiredSuites = [
  { id: 'docs-structure',    file: 'rules/docs/structure.test.mjs' },
  { id: 'git-commits',       file: 'rules/git/commits.test.mjs' },
  // …
];
```

- El **runner** valida que toda suite del manifiesto haya sido descubierta, que
  su `id` corresponda exactamente al `file` declarado, que no haya duplicados y
  que no existan suites fuera del manifiesto.
- La suite `docs-structure` comprueba además que el **archivo** de cada entrada
  exista (test etiquetado R-CI-7).

La suite `microservices-ci-contract` deriva la matriz esperada de `SERVICES` y
`SERVICE_TEST_EXCEPTIONS` (`config/project.mjs`). Así, agregar un servicio sin
incorporarlo al build/test raíz, omitir su configuración Vitest o mantener una
excepción después de crear sus specs convierte la deriva en un fallo de CI.
También compara `config/coverage.mjs` con el baseline de
`docs/rules/test/06-estandares-cobertura.md` para que ningún umbral baje
(R-COV-1).

## Reporte de cobertura

Al terminar, el runner imprime cuántos IDs de regla tienen check ejecutable y
**expone un hallazgo** cuando una suite cita un ID que no está definido en
`docs/rules`:

```
Cobertura ejecutable: 30/238 IDs de regla tienen check.
```

> El reporte nació exponiendo un hallazgo real: `R-CI-9` y `R-CD-22` se citaban
> en los scripts sin existir como reglas. Se resolvió definiendo **R-CI-7**
> (manifiesto de suites) y **R-CD-14** (higiene de imagen) y ajustando las
> referencias; hoy no hay hallazgos pendientes.

## CLI

```
node scripts/cli.mjs rules [--only <id|area>] [--mode pure|repo] [--reporter spec|tap|dot|junit]
node scripts/cli.mjs smoke
node scripts/cli.mjs swagger
```

Los `npm scripts` de la raíz son **envoltorios** (sin ruptura para CI ni local):

| npm script | Envuelve |
|---|---|
| `rules:check` | `node scripts/cli.mjs rules` |
| `release:check` | `node scripts/tools/release-check.mjs` |
| `version:bump` | `node scripts/tools/version-bump.mjs` |
| `coverage:compare` | `node scripts/tools/coverage-compare.mjs` |
| `coverage:baseline` | `node scripts/tools/coverage-baseline.mjs` |
| `demo` | `node scripts/cli.mjs smoke` |
| `swagger:export` | `node scripts/cli.mjs swagger` |
| `test:engine` | autotests del motor, incluida una suite centinela que prueba el exit code ante assertions fallidas |
| `test:smoke` | regresiones del runner, del agregador y del contrato OpenAPI (superficies, seguridad y respuestas); ejecutadas en CI |

`npm run demo` sigue ejercitando el stack completo levantado. El runner usa
`SMOKE_REQUEST_TIMEOUT_MS` (10 000 ms) por petición y `SMOKE_WAIT_TIMEOUT_MS`
(30 000 ms) por espera asíncrona. Reintenta una sola vez las peticiones GET
si falla la red; las mutaciones no se repiten sin clave de idempotencia.
`test:smoke` prueba ese contrato con un servidor HTTP efímero y no sustituye
el gate E2E pendiente (G-6).

## Cómo añadir una regla ejecutable

1. Identifica el **dominio** y crea/edita su suite en `rules/<dominio>/`.
2. Declara la suite con `defineSuite` (incluye el/los ID(s) en `rules`).
3. Añade `{ id, file }` al `requiredSuites` del `manifest.mjs`.
4. Ejecuta `npm run rules:check` en verde.
5. La regla normativa sigue viviendo en `docs/rules/<area>/NN-tema.md`; el motor
   sólo la **verifica**. La regla y su check entran en el **mismo PR**.

## Estado

El destino lo fija [ADR-23](../docs/decisiones.md) y la migración se completó en
sus cuatro fases: `engine/lib/*` + `tools/` (fase 1 y 4), `config/*` + `rules/*`
por dominio (fase 2), `runner` + `suite` + `cli` (fase 3), y lint + autotests del
motor (fase 4).

> El estado de cumplimiento (deuda, auditorías) **no vive aquí**: vive en
> [`docs/rules/_meta/AUDIT.md`](../docs/rules/_meta/AUDIT.md) (R-DO-7).
