# ADR-0044 — Backup lógico, retención y verificación de restore

- **Fecha:** 2026-11-20
- **Estado:** Aceptado (implementado el backup y el restore drill; PITR queda en backlog)
- **Relacionado:** [ADR-0042](./0042-exportacion-asincrona-purga-total-pdf-storage.md),
  [ADR-0046](./0046-suscripcion-plataforma-expansion.md)

## Contexto

La auditoría de plataforma confirmó que el proyecto no tenía backups
automatizados: solo existían menciones sueltas a "confirmar backup" en runbooks
de migración y un rollback de despliegue (R-CD-13) que redespliega la imagen
anterior, no recupera datos. Para un SaaS multi-tenant con PostgreSQL como
fuente de verdad, eso es un riesgo de pérdida de datos, no una mejora de escala.

## Decisión

1. **Backup lógico diario.**
   - `infra/db/backup.sh` ejecuta `pg_dump --format=custom --no-owner
     --no-privileges`, escribe el dump y su SHA-256 en `BACKUP_DIR` y aplica
     retención por días (`BACKUP_RETENCION_DIAS`, por defecto 14).
   - El compose de dev, stage y producción levanta un sidecar `db-backup` con
     `postgres:16-alpine` que repite el dump cada
     `BACKUP_INTERVALO_SEGUNDOS` (por defecto 24 h) sobre el volumen
     `db_backups`.
2. **Copia fuera del host.**
   - `.github/workflows/backup.yml` ejecuta el mismo script contra
     `BACKUP_DATABASE_URL`, sube el dump como artifact (30 días) y, en el mismo
     run, lo restaura en un PostgreSQL efímero con PostGIS para verificar que el
     dump es utilizable.
   - El workflow queda como `workflow_dispatch` hasta que el secret exista; el
     sidecar sigue siendo la automatización local.
3. **Verificación de restore.**
   - `infra/db/restore-verify.sh` crea una base temporal, restaura con
     `pg_restore`, comprueba que existen los esquemas esperados y la elimina.
   - Nunca restaura sobre la base de origen.
4. **Objetivos operativos.**
   - **RPO:** 24 h con el dump diario (el artifact de CI añade una segunda copia).
   - **RTO:** 2 h para restaurar un dump reciente en un PostgreSQL equivalente.
   - El runbook vive en [`docs/runbook-backup-restore.md`](../runbook-backup-restore.md).
5. **PITR (point-in-time recovery) no se activa en este ADR.**
   - Requiere `wal_level=replica`, `archive_mode=on`, archivado de WAL fuera del
     host y una base de respaldo (`pg_basebackup`/pgBackRest), además de una
     ventana de mantenimiento y una prueba de restore concreta.
   - Se documenta como deuda P1 en
     [`docs/backlog-plataforma.md`](../backlog-plataforma.md); el dump lógico
     diario cubre el MVP y no altera la configuración del PostgreSQL existente.

## Consecuencias

- **Positivas:** hay una copia automática diaria, checksum, retención y una
  verificación de restore reproducible. El rollback deja de ser la única
  respuesta ante corrupción de datos.
- **Limitación:** el volumen `db_backups` vive en el mismo host; sin la copia de
  CI o una sincronización externa no protege contra pérdida del VPS. El runbook
  exige configurar una de las dos para producción.
- **Limitación:** el RPO de 24 h puede ser insuficiente para un tenant con alto
  volumen; PITR o dumps más frecuentes son la evolución natural.
- **Operación:** el sidecar consume disco; la retención debe ajustarse al tamaño
  real de la base y al espacio del volumen.

## Criterios de aceptación

- Un dump diario aparece en `db_backups` con su `.sha256`.
- `restore-verify.sh` restaura el dump en una base temporal y valida los
  esquemas `identity`, `orders`, `finance` y `notifications`.
- El workflow manual produce un artifact y supera el restore drill.
- El runbook describe qué hacer si el dump falla, cómo restaurar y cómo
  escalar a PITR.
