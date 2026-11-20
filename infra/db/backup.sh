#!/bin/sh
# Backup lógico diario de PostgreSQL (ADR-0044).
#
# Uso:
#   DATABASE_URL=postgres://... BACKUP_DIR=/backups sh infra/db/backup.sh
#   PGHOST=postgres PGUSER=core_engine PGPASSWORD=... sh infra/db/backup.sh
#
# Genera un dump custom (pg_dump -Fc), su SHA-256 y aplica retención local.
# El envío fuera del host es responsabilidad del workflow de CI o del almacenamiento
# externo configurado por operación (ver docs/runbook-backup-restore.md).

set -eu

BACKUP_DIR="${BACKUP_DIR:-/backups}"
BACKUP_RETENCION_DIAS="${BACKUP_RETENCION_DIAS:-14}"
BACKUP_PREFIJO="${BACKUP_PREFIJO:-core_engine}"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
NOMBRE="${BACKUP_PREFIJO}_${TIMESTAMP}.dump"
DESTINO="${BACKUP_DIR}/${NOMBRE}"
TEMPORAL="${DESTINO}.tmp"

mkdir -p "$BACKUP_DIR"

if [ -n "${DATABASE_URL:-}" ]; then
  pg_dump --format=custom --no-owner --no-privileges --file="$TEMPORAL" "$DATABASE_URL"
else
  : "${PGHOST:?PGHOST o DATABASE_URL es obligatorio}"
  : "${PGDATABASE:?PGDATABASE es obligatorio}"
  : "${PGUSER:?PGUSER es obligatorio}"
  pg_dump --format=custom --no-owner --no-privileges --file="$TEMPORAL"
fi

mv "$TEMPORAL" "$DESTINO"
sha256sum "$DESTINO" > "${DESTINO}.sha256"

if [ "${BACKUP_RETENCION_DIAS}" -gt 0 ] 2>/dev/null; then
  find "$BACKUP_DIR" -type f \
    \( -name "${BACKUP_PREFIJO}_*.dump" -o -name "${BACKUP_PREFIJO}_*.dump.sha256" \) \
    -mtime "+${BACKUP_RETENCION_DIAS}" -delete
fi

echo "backup: ${DESTINO} ($(du -h "$DESTINO" | cut -f1), retención ${BACKUP_RETENCION_DIAS}d)"
