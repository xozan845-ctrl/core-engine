# Reglas de Microservicio — Core Engine

Reglas doradas de **estructura** de los microservicios (layout de `packages/`,
separación de capas, DI, DTOs, fail-fast y graceful shutdown) y de las
**excepciones** que algunos servicios tienen frente a las reglas transaccionales
globales (hoy: la excepción OLAP de Inteligencia de Mercado).

## Archivos

| Archivo | Contenido | Sufijo |
|---|---|---|
| `01-estructura.md` | Ubicación de código, clean architecture interna, DI, DTOs, env, shutdown | `R-MS` (1–6) |
| `02-excepcion-olap.md` | Excepción OLAP de `market-intelligence-service`: CQRS analítico, PostGIS, data contracts, Kafka fase 2, feature store | `R-MI` (1–5) |

> Los documentos derivados de este área (**AUDIT, CHECKLIST, historiales**) no
> viven aquí: están en [`../_meta/`](../_meta/README.md). Un área de reglas solo
> contiene `README.md` + archivos `NN-tema.md`.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-MS-*` | 1–6 | `01-estructura.md` | Estructura y ciclo de vida de un servicio |
| `R-MI-*` | 1–5 | `02-excepcion-olap.md` | Excepción analítica (OLAP) de market-intelligence |

## Reglas de otras áreas que exige esta (cross-refs)

| ID | Exigencia |
|---|---|
| `R-MS-2` | Sin mutaciones críticas por HTTP síncrono → `R-AR-2` (`../architecture/01-arquitectura.md`) |
| `R-MS-4` | DTO ≠ entidad → `R-C-3` (`../test/04-reglas-contrato.md`) |
| `R-MS-5` | Fail-fast de env → `R-DS-1` (`../data/02-seguridad.md`) |
| `R-MI-1` | Única excepción a `R-AR-1` (CRUD convencional por defecto) |

## Pendientes

- Pendiente de auditar: entra en la próxima revisión (`R-COV-3`).
