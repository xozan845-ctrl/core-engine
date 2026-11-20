# Runbook — backup, restore y recuperación

Cubre el backup lógico diario, la verificación de restore y la respuesta ante
pérdida o corrupción de datos. Decisión: [ADR-0044](./adr/0044-backup-restore-verificado.md).

## 1. Objetivos

| Métrica | Valor MVP | Nota |
|---|---|---|
| RPO | 24 h | Dump diario; el artifact de CI es una segunda copia |
| RTO | 2 h | Restore de un dump reciente en un PostgreSQL equivalente |
| Retención local | 14 días | `BACKUP_RETENCION_DIAS` |
| Retención offsite | 30 días | Artifact del workflow |
| Verificación | Cada backup manual/CI | `restore-verify.sh` |

**PITR no está activo.** El dump lógico recupera hasta el último backup, no
hasta el último segundo. Ver §7.

## 2. Componentes

- `infra/db/backup.sh`: dump custom + SHA-256 + retención.
- `infra/db/restore-verify.sh`: restore en base temporal + validación.
- Sidecar `db-backup` en `docker-compose{,.dev,.stage}.yml`: ejecuta el dump
  cada `BACKUP_INTERVALO_SEGUNDOS` (24 h por defecto) en el volumen `db_backups`.
- `.github/workflows/backup.yml`: copia offsite y restore drill en un runner.
- `docs/adr/0044-backup-restore-verificado.md`: decisión y límites.

## 3. Verificación diaria

```bash
# Ver el último dump y su checksum
docker compose exec db-backup ls -lh /backups
docker compose exec db-backup sh -c 'cd /backups && sha256sum -c *.sha256'

# Forzar un backup sin esperar al ciclo
docker compose exec db-backup sh /usr/local/bin/backup.sh
```

Si el sidecar no está corriendo:

```bash
docker compose ps db-backup
docker compose logs --tail=100 db-backup
docker compose up -d db-backup
```

## 4. Restore drill (obligatorio antes de producción y mensual)

Nunca restaurar sobre la base de origen. El script crea y elimina una base
temporal:

```bash
docker compose exec db-backup sh /usr/local/bin/restore-verify.sh \
  /backups/core_engine_20260101T030000Z.dump
```

Resultado esperado: `restore verificado: base=... empresas=... esquemas=...`.

Para conservar la base temporal e inspeccionarla:

```bash
docker compose exec -e RESTORE_KEEP=1 db-backup \
  sh /usr/local/bin/restore-verify.sh /backups/<archivo>.dump
```

## 5. Restauración real (incidente)

1. **Declarar incidente** y congelar escrituras (escala el gateway o pon la API
   en mantenimiento) para no perder cambios posteriores al dump.
2. **Elegir el dump** más reciente y verificar su SHA-256.
3. **Crear una base nueva** (no sobrescribir la dañada):
   ```bash
   createdb core_engine_restore
   pg_restore --no-owner --no-privileges --dbname=core_engine_restore <archivo>.dump
   ```
4. **Validar** conteos mínimos (`identity.empresas`, `orders.ordenes`,
   `finance.asientos`) y un login de smoke.
5. **Cambiar `DATABASE_URL`** al restore y reiniciar servicios, o renombrar las
   bases en una ventana controlada.
6. **Reabrir tráfico**, ejecutar smoke y registrar el incidente.
7. **Preservar** la base dañada para análisis; no borrarla hasta cerrar el
   postmortem.

## 6. Copia fuera del host

El volumen `db_backups` está en el mismo VPS: no protege contra pérdida del
host. Para producción hay que elegir una:

- ejecutar el workflow `Database Backup` (requiere `BACKUP_DATABASE_URL` y
  acceso de red desde el runner; si la BD es privada, usar un runner
  self-hosted en la red), o
- sincronizar `db_backups` a un bucket/S3 compatible con `rclone`/`mc` desde
  operación.

Sin una de las dos, el backup no cumple DR.

## 7. PITR (pendiente)

Para reducir el RPO a minutos hace falta:

1. `wal_level=replica` y `archive_mode=on` en PostgreSQL.
2. `archive_command` que copie WAL a un volumen/objeto fuera del host.
3. `pg_basebackup` periódico (o pgBackRest).
4. Un restore drill que aplique base + WAL hasta un timestamp.

No se activa en el MVP porque cambia la configuración del PostgreSQL existente y
exige una ventana de mantenimiento. Está en
[`docs/backlog-plataforma.md`](./backlog-plataforma.md).

## 8. Fallos frecuentes

| Síntoma | Causa probable | Acción |
|---|---|---|
| `pg_dump: error: connection ...` | credenciales/red del sidecar | revisar `PG*` y `docker compose logs postgres` |
| Dump de 0 bytes | disco lleno o `pg_dump` interrumpido | liberar espacio, borrar `.tmp`, reintentar |
| `restore-verify` falla por extensión | falta PostGIS/contrib en el destino | usar `postgis/postgis:16-3.4` |
| `sha256sum -c` falla | dump corrupto | descartar y usar el artifact anterior |
| No hay dumps | sidecar detenido | `docker compose up -d db-backup` |

## 9. Responsabilidad

- **Operación:** revisar el sidecar y el artifact; ejecutar el restore drill
  mensual.
- **Backend:** mantener scripts/workflow y actualizar este runbook si cambia el
  esquema o el proveedor de PostgreSQL.
- **Antes de producción:** el restore drill debe estar ejecutado y documentado en
  el PR de promoción.
