#!/bin/sh
# Aplica migraciones SQL versionadas con ledger idempotente.
#
# - Crea `public.schema_migrations` con nombre + checksum SHA-256.
# - Si la base ya existe pero el ledger está vacío (bootstrap de un entorno
#   creado con `docker-entrypoint-initdb.d`), marca las migraciones actuales
#   como aplicadas sin re-ejecutarlas.
# - Aplica en orden lexicográfico las pendientes y falla si una ya aplicada
#   cambió de contenido (drift).
#
# Uso: PGHOST=postgres PGUSER=... PGPASSWORD=... PGDATABASE=... sh apply.sh
set -eu

DIR="${MIGRATIONS_DIR:-/migrations}"
INIT_DIR="${DIR}/init"
PSQL="psql -v ON_ERROR_STOP=1 --no-psqlrc"

: "${PGHOST:?PGHOST es obligatorio}"
: "${PGUSER:?PGUSER es obligatorio}"
: "${PGDATABASE:?PGDATABASE es obligatorio}"

$PSQL -q -c "
  CREATE TABLE IF NOT EXISTS public.schema_migrations (
    nombre      text PRIMARY KEY,
    checksum    text NOT NULL,
    aplicado_en timestamptz NOT NULL DEFAULT NOW()
  );"

if [ ! -d "$INIT_DIR" ]; then
  echo "migraciones: no existe $INIT_DIR" >&2
  exit 1
fi

LEDGER_VACIO="$($PSQL -At -c 'SELECT COUNT(*) = 0 FROM public.schema_migrations')"
BASE_EXISTENTE="$($PSQL -At -c "SELECT to_regclass('identity.empresas') IS NOT NULL")"

if [ "$LEDGER_VACIO" = "t" ] && [ "$BASE_EXISTENTE" = "t" ]; then
  echo "migraciones: base existente sin ledger; se registra el baseline"
  for archivo in "$INIT_DIR"/*.sql; do
    [ -f "$archivo" ] || continue
    nombre="$(basename "$archivo")"
    checksum="$(sha256sum "$archivo" | cut -d' ' -f1)"
    $PSQL -q -c "
      INSERT INTO public.schema_migrations (nombre, checksum)
      VALUES ('$nombre', '$checksum')
      ON CONFLICT (nombre) DO NOTHING;"
  done
fi

aplicadas=0
for archivo in "$INIT_DIR"/*.sql; do
  [ -f "$archivo" ] || continue
  nombre="$(basename "$archivo")"
  checksum="$(sha256sum "$archivo" | cut -d' ' -f1)"
  registrado="$($PSQL -At -c \
    "SELECT checksum FROM public.schema_migrations WHERE nombre = '$nombre'")"
  if [ -n "$registrado" ]; then
    if [ "$registrado" != "$checksum" ]; then
      echo "migraciones: $nombre cambió después de aplicarse (drift)" >&2
      exit 1
    fi
    continue
  fi
  echo "migraciones: aplicando $nombre"
  $PSQL -f "$archivo"
  $PSQL -q -c "
    INSERT INTO public.schema_migrations (nombre, checksum)
    VALUES ('$nombre', '$checksum');"
  aplicadas=$((aplicadas + 1))
done

# RLS de Supabase: se aplica al final y solo si el archivo está presente.
RLS="${DIR}/supabase/99_rls.sql"
if [ -f "$RLS" ] && [ "${APPLY_RLS:-false}" = "true" ]; then
  checksum="$(sha256sum "$RLS" | cut -d' ' -f1)"
  registrado="$($PSQL -At -c \
    "SELECT checksum FROM public.schema_migrations WHERE nombre = '99_rls.sql'")"
  if [ -z "$registrado" ]; then
    echo "migraciones: aplicando 99_rls.sql"
    $PSQL -f "$RLS"
    $PSQL -q -c "
      INSERT INTO public.schema_migrations (nombre, checksum)
      VALUES ('99_rls.sql', '$checksum');"
    aplicadas=$((aplicadas + 1))
  elif [ "$registrado" != "$checksum" ]; then
    echo "migraciones: 99_rls.sql cambió después de aplicarse (drift)" >&2
    exit 1
  fi
fi

echo "migraciones: OK ($aplicadas aplicadas)"
