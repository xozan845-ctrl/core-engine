# Reglas de Microservicio — Core Engine

Reglas doradas de **estructura** de los microservicios (layout de `packages/`,
separación de capas, DI, DTOs, fail-fast y graceful shutdown) y de las
**excepciones** que algunos servicios tienen frente a las reglas transaccionales
globales (hoy: la excepción OLAP de Inteligencia de Mercado).

## Archivos

| Archivo | Contenido | Sufijo |
|---|---|---|
| `01-estructura.md` | Layout del crate Rust, capas internas, DI por `AppState`, DTOs, env fail-fast, shutdown, división de módulos y README por paquete | `R-MS` (1–8) |
| `02-excepcion-olap.md` | Excepción OLAP de `market-intelligence-service`: CQRS analítico, PostGIS, data contracts, Kafka fase 2, feature store | `R-MI` (1–5) |

> Los documentos derivados de este área (**AUDIT, CHECKLIST, historiales**) no
> viven aquí: están en [`../_meta/`](../_meta/README.md). Un área de reglas solo
> contiene `README.md` + archivos `NN-tema.md`.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-MS-*` | 1–8 | `01-estructura.md` | Estructura, modularidad y ciclo de vida de un servicio |
| `R-MI-*` | 1–5 | `02-excepcion-olap.md` | Excepción analítica (OLAP) de market-intelligence |

## Reglas de otras áreas que exige esta (cross-refs)

| ID | Exigencia |
|---|---|
| `R-MS-2` | Sin mutaciones críticas por HTTP síncrono → `R-AR-2` (`../architecture/01-arquitectura.md`) |
| `R-MS-4` | DTO ≠ entidad → `R-C-3` (`../test/04-reglas-contrato.md`) |
| `R-MS-5` | Fail-fast de env → `R-DS-1` (`../data/02-seguridad.md`) |
| `R-MI-1` | Única excepción a `R-AR-1` (CRUD convencional por defecto) |

## Verificación

- `01-estructura.md` se audita en cada revisión de `R-COV-3`; la suite
  `microservices-ci-contract` valida que cada servicio declare su crate, su
  Dockerfile y su README.
- `R-MS-7` tiene check propio en la suite `microservices-tamano-modulos`: falla
  si un archivo de `packages/*/src` supera los presupuestos de responsabilidad
  (800 líneas de producción, 20 funciones públicas en dominio con SQL u 8 tablas
  SQL) y `baseline:guard` impide añadir deuda nueva o relajar los presupuestos.
