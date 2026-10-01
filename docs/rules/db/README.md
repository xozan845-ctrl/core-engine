# Reglas de DB — Core Engine

Reglas doradas de **base de datos**: aislamiento por esquema, RLS,
transaccionalidad, índices, soft delete, migraciones y paginación; más las
reglas específicas de **Supabase** (paridad de entornos, Auth, secretos) que
aplican en staging/producción.

> ¿Integridad lógica de los datos (dinero, stock, idempotencia) y seguridad de
> accesos? Van en [`../data/`](../data/README.md). Este área trata el
> **almacenamiento**.

## Archivos

| Archivo | Contenido | Sufijo |
|---|---|---|
| `01-database.md` | Schema-per-service, RLS, transacciones, índices, soft delete, migraciones, paginación | `R-DB` (1–7) |
| `02-supabase.md` | Paridad de entornos, RLS obligatorio, `set_config`, migraciones, secretos, Storage | `R-SB` (1–6) |

> Los documentos derivados de este área (**AUDIT, CHECKLIST, historiales**) no
> viven aquí: están en [`../_meta/`](../_meta/README.md). Un área de reglas solo
> contiene `README.md` + archivos `NN-tema.md`.

## Mapa de IDs

| Prefijo | Rango | Archivo | Ámbito |
|---|---|---|---|
| `R-DB-*` | 1–7 | `01-database.md` | Reglas transaccionales de Postgres |
| `R-SB-*` | 1–6 | `02-supabase.md` | Supabase: RLS, Auth, migraciones y secretos |

## Reglas de otras áreas que exige esta (cross-refs)

| ID | Exigencia |
|---|---|
| `R-DB-2/3` | RLS y outbox en la misma transacción → `R-AR-3` (`../architecture/01-arquitectura.md`) |
| `R-DB-5` | Soft delete ≠ `TRUNCATE` de tests → `R-I-10`/`R-E-11` (`../test/`) |
| `R-DB-6` | Migraciones versionadas → `R-CD-5` (`../cd/01-despliegue.md`) |
| `R-SB-5` | Secretos solo en env → `R-DS-1` (`../data/02-seguridad.md`) |

## Pendientes

- Pendiente de auditar: entra en la próxima revisión (`R-COV-3`).
