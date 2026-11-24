#!/bin/sh
# Verificación de restore de un dump de PostgreSQL (ADR-0044).
#
# Restaura el dump en una base temporal, ejecuta comprobaciones mínimas y la
# elimina. Nunca toca la base de origen.
#
# Uso:
#   PGHOST=localhost PGUSER=postgres PGPASSWORD=... \
#     sh infra/db/restore-verify.sh /backups/core_engine_20260101T030000Z.dump
#
# Requiere permisos de CREATEDB. `RESTORE_KEEP=1` conserva la base temporal para
# inspección manual.

set -eu

DUMP="${1:?Uso: restore-verify.sh <archivo.dump>}"
: "${PGHOST:?PGHOST es obligatorio}"
: "${PGUSER:?PGUSER es obligatorio}"
: "${PGDATABASE:?PGDATABASE es obligatorio}"

# Verifica el checksum cuando el backup lo generó.
if [ -f "${DUMP}.sha256" ]; then
  (cd "$(dirname "$DUMP")" && sha256sum -c "$(basename "$DUMP").sha256")
fi

# El dump usa --no-privileges; las políticas RLS referencian estos roles, así
# que deben existir antes de restaurar en un clúster limpio.
psql -v ON_ERROR_STOP=1 -d "$PGDATABASE" -c "
DO \$\$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
    CREATE ROLE anon NOLOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
    CREATE ROLE authenticated NOLOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    CREATE ROLE core_engine_app NOLOGIN;
  END IF;
  GRANT authenticated TO core_engine_app;
END \$\$;" >/dev/null

BASE="core_engine_restore_$(date -u +%Y%m%d%H%M%S)_$$"
LOG="/tmp/restore-${BASE}.log"

cleanup() {
  if [ "${RESTORE_KEEP:-0}" != "1" ]; then
    dropdb --if-exists --force "$BASE" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT INT TERM

createdb "$BASE"
pg_restore --exit-on-error --no-owner --no-privileges --dbname="$BASE" "$DUMP" >"$LOG" 2>&1 || {
  echo "restore falló; log: $LOG" >&2
  exit 1
}

EMPRESAS="$(psql -At -d "$BASE" -c "SELECT COUNT(*) FROM identity.empresas" 2>/dev/null || echo "?")"
ESQUEMAS="$(psql -At -d "$BASE" -c "SELECT COUNT(*) FROM information_schema.schemata WHERE schema_name IN ('identity','orders','finance','notifications','logistics')")"
TABLAS="$(psql -At -d "$BASE" -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema IN ('identity','orders','finance','notifications','logistics','storage','item','inventory','pricing','stores','scheduling','commissions','field','analytics','intelligence')")"
CON_RLS="$(psql -At -d "$BASE" -c "SELECT COUNT(*) FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace WHERE c.relkind = 'r' AND c.relrowsecurity AND n.nspname IN ('identity','orders','finance','notifications','logistics','storage','item','inventory','pricing','stores','scheduling','commissions','field','analytics','intelligence')")"

if [ "$ESQUEMAS" -lt 3 ]; then
  echo "restore incompleto: solo $ESQUEMAS esquemas esperados" >&2
  exit 1
fi
if [ "$EMPRESAS" = "?" ] || [ "$TABLAS" -lt 100 ]; then
  echo "restore incompleto: empresas=$EMPRESAS tablas=$TABLAS" >&2
  exit 1
fi
if [ "$CON_RLS" -lt 20 ]; then
  echo "restore sin RLS: tablas_con_rls=$CON_RLS" >&2
  exit 1
fi

echo "restore verificado: base=$BASE empresas=$EMPRESAS esquemas=$ESQUEMAS tablas=$TABLAS con_rls=$CON_RLS"
