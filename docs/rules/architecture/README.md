# Reglas de Arquitectura — Core Engine

Reglas doradas del estilo arquitectónico y la nomenclatura: cómo se
comunican los servicios (CQRS, outbox, DLQ, coreografía), cómo se resuelven
las dependencias (autonomía, ACL, caché con TTL) y cómo se nombran archivos,
clases, tablas y commits. Es el área de la que cuelgan referencias casi todas
las demás (`R-AR-2` es la regla más citada del repo).

## Archivos

| Archivo | Contenido | Sufijo |
|---|---|---|
| `01-arquitectura.md` | CQRS/Event Sourcing por dominio, coreografía vs HTTP síncrono, outbox, DLQ, autonomía, ACL, caché con TTL | `R-AR` (1–7) |
| `02-naming.md` | Archivos y carpetas, sufijos NestJS/CQRS, clases/DTOs, camelCase, `snake_case`, idioma, Conventional Commits | `R-NC` (1–8) |

> Los documentos derivados de este área (**AUDIT, CHECKLIST, historiales**) no
> viven aquí: están en [`../_meta/`](../_meta/README.md). Un área de reglas solo
> contiene `README.md` + archivos `NN-tema.md`.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-AR-*` | 1–7 | `01-arquitectura.md` | Patrones de comunicación, resiliencia y caché |
| `R-NC-*` | 1–8 | `02-naming.md` | Nomenclatura de código, BD y commits |

## Reglas de otras áreas que exige esta (cross-refs)

| ID | Exigencia |
|---|---|
| `R-AR-3` | Outbox en la misma transacción → ver `R-DB-3` (`../db/01-database.md`) |
| `R-AR-4` | Reintentos + DLQ → alerta de crecimiento sostenido en `R-TM-5` (`../observability/02-telemetria.md`) |
| `R-AR-7` | TTL de caché ≠ TTL de claves de idempotencia → `R-DI-4` (`../data/01-integridad.md`) |
| `R-NC-8` | Commits → `R-GC-1..7` (`../git/02-commits.md`) |

## Pendientes

- Pendiente de auditar: entra en la próxima revisión (`R-COV-3`).
