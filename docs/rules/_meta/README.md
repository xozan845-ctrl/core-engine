# _meta/ — Documentos derivados (NO son reglas)

Carpetas marcadas con `_` = **no normativa**. Nada de aquí obliga: todo se deriva de
las reglas de las áreas (`../architecture/`, `../microservice/`, `../gateway/`,
`../data/`, `../db/`, `../observability/`, `../test/`, `../git/`, `../ci/`)
y se actualiza cuando éstas cambian.

| Archivo | Qué es | Cuándo se actualiza |
|---|---|---|
| `CHECKLIST.md` | Herramienta de revisión de PR derivada de las reglas (remite a IDs) | Cuando cambian reglas que afecten revisión |
| `AUDIT.md` | **Snapshot** del estado actual de cumplimiento — se reescribe completo | En cada auditoría (antes de cada release, R-COV-3) |
| `AUDIT-HISTORY.md` | Log **append-only** del histórico de auditorías (con procedencia del proyecto origen) | Nunca se reescribe: solo se agrega la entrada nueva |

> **Historia (cerrada 2026-10-01):** la deuda que este README declaraba —
> *"`CHECKLIST.md`, `AUDIT.md` y `AUDIT-HISTORY.md` aún reflejan el proyecto
> origen… se reescriben completos en la primera auditoría de Core Engine"* — se
> cerró en esa primera auditoría (R-COV-3): `AUDIT.md` y `CHECKLIST.md`
> reescritos a este repo, y `AUDIT-FRONTEND.md` borrado tal como preveía su
> fila de la tabla.

## Reglas de estructura

1. Los documentos de esta carpeta **jamás** contienen reglas propias: solo resumen,
   estado o historial de las reglas que viven en las áreas.
2. Si un archivo derivado y su regla de origen discrepan, **manda la regla** (y se
   corrige el derivado).
3. Prohibido mover un audit/checklist dentro de un área: el área es 100% reglas.
