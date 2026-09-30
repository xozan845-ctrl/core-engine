# Reglas de Data — Core Engine

Reglas doradas sobre los **datos** como activo: integridad (dinero en centavos,
stock atómico, máquinas de estados, idempotencia, partida doble) y seguridad
(secretos, PII, mínimo privilegio, validación de entradas, tráfico interno,
rate limiting, dependencias).

> ¿Esquemas, migraciones, índices y RLS? Van en [`../db/`](../db/README.md).
> Este área trata la **lógica de los datos**, no el almacenamiento.

## Archivos

| Archivo | Contenido | Sufijo |
|---|---|---|
| `01-integridad.md` | Montos en centavos, stock atómico, máquina de estados, idempotencia, partida doble, deadlocks | `R-DI` (1–6) |
| `02-seguridad.md` | Secretos, PII, mínimo privilegio, DTOs, `x-internal-key`, rate limiting, `npm audit` | `R-DS` (1–7) |

> Los documentos derivados de este área (**AUDIT, CHECKLIST, historiales**) no
> viven aquí: están en [`../_meta/`](../_meta/README.md). Un área de reglas solo
> contiene `README.md` + archivos `NN-tema.md`.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-DI-*` | 1–6 | `01-integridad.md` | Integridad y consistencia de datos |
| `R-DS-*` | 1–7 | `02-seguridad.md` | Seguridad de datos y accesos |

## Reglas de otras áreas que exige esta (cross-refs)

| ID | Exigencia |
|---|---|
| `R-DI-1` | Monedas → clase `Money` de `@core/shared` (ADR-01) |
| `R-DI-4` | Idempotencia ≠ caché: TTL mínimo 1h/24h, ver `R-AR-7` (`../architecture/01-arquitectura.md`) |
| `R-DS-5` | Scrub de `x-internal-key` en el borde → `R-GW-3` (`../gateway/01-auth.md`) |
| `R-DS-6` | Rate limiting vive solo en el gateway → `R-GW-2` (`../gateway/01-auth.md`) |
| `R-DS-7` | Gate de seguridad en CI → `R-QA-6` (`../test/07-qa-gates.md`) |

## Pendientes

- Pendiente de auditar: entra en la próxima revisión (`R-COV-3`).
