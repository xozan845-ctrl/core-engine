# Runbook — PITR y recuperación a un punto en el tiempo

Estado actual: **PITR no activo**. El backup es un dump lógico diario (RPO 24 h)
y el restore drill está reparado. Este documento fija cómo activarlo y qué
verificar; no se activa en el MVP sin ventana de mantenimiento y drill.

## 1. Requisitos

- PostgreSQL con `wal_level=replica` (ya configurado en el compose de stage/prod).
- Almacenamiento de WAL fuera del host (S3/B2/MinIO distinto del de la app).
- `pgBackRest` o `WAL-G` en el host o en un sidecar.
- Espacio para base + WAL; el archivado no puede quedar en el mismo volumen.

## 2. Activación (pgBackRest)

1. Instalar `pgbackrest` en el host y crear `/etc/pgbackrest/pgbackrest.conf` con
   el repositorio S3 y la retención (p. ej. `repo1-retention-full=14`).
2. En PostgreSQL:
   ```conf
   archive_mode = on
   archive_command = 'pgbackrest --stanza=core-engine archive-push %p'
   ```
3. Reiniciar PostgreSQL en ventana controlada.
4. Crear la stanza y el primer full:
   ```bash
   pgbackrest --stanza=core-engine stanza-create
   pgbackrest --stanza=core-engine --type=full backup
   ```
5. Programar un full semanal y diferenciales diarios.

## 3. Restore a un timestamp

```bash
pgbackrest --stanza=core-engine --type=time \
  --target="2026-11-23 12:00:00-06" \
  --target-action=promote restore
```

Restaurar en una instancia nueva, validar conteos y login, y solo después
cambiar `DATABASE_URL`.

## 4. Drill obligatorio

- Mensual: restaurar el último full + WAL hasta un minuto concreto en staging.
- Medir RTO real (objetivo < 2 h) y RPO real (objetivo < 15 min).
- Documentar el resultado en el ticket de operación.
- Si el drill falla, no se declara PITR activo.

## 5. Rollback

Mientras PITR no esté validado, el rollback es el dump lógico + restore en base
nueva (`restore-verify.sh`). No revertir migraciones.
