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

# El rol de aplicación debe poder iniciar sesión; en bases creadas antes de
# este runner podía quedar NOLOGIN (solo lo creaba 00_roles.sh).
if [ -n "${CORE_ENGINE_APP_PASSWORD:-}" ]; then
  APP_PWD="$(printf '%s' "$CORE_ENGINE_APP_PASSWORD" | sed "s/'/''/g")"
  $PSQL -q -c "
    DO \$\$
    BEGIN
      IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
        CREATE ROLE core_engine_app NOLOGIN;
      END IF;
      IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
        CREATE ROLE authenticated NOLOGIN;
      END IF;
      GRANT authenticated TO core_engine_app;
    END \$\$;"
  printf "ALTER ROLE core_engine_app LOGIN PASSWORD '%s';\n" "$APP_PWD" | $PSQL -q
  $PSQL -q -c "ALTER ROLE core_engine_app NOSUPERUSER NOBYPASSRLS;"
fi

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
  $PSQL -q -c "SET request.jwt.claims = '{\"rol\":\"super_admin\"}'" -f "$archivo"
  $PSQL -q -c "
    INSERT INTO public.schema_migrations (nombre, checksum)
    VALUES ('$nombre', '$checksum');"
  aplicadas=$((aplicadas + 1))
done

# RLS es obligatorio. Se aplica al final, después de todas las tablas, y luego
# un endurecimiento que elimina bypass, activa FORCE RLS y cubre tablas nuevas.
for archivo in "${DIR}/supabase/99_rls.sql" "${DIR}/supabase/99z_rls_hardening.sql"; do
  [ -f "$archivo" ] || continue
  nombre="$(basename "$archivo")"
  checksum="$(sha256sum "$archivo" | cut -d' ' -f1)"
  registrado="$($PSQL -At -c \
    "SELECT checksum FROM public.schema_migrations WHERE nombre = '$nombre'")"
  if [ -z "$registrado" ]; then
    echo "migraciones: aplicando $nombre"
    $PSQL -q -c "SET request.jwt.claims = '{\"rol\":\"super_admin\"}'" -f "$archivo"
    $PSQL -q -c "
      INSERT INTO public.schema_migrations (nombre, checksum)
      VALUES ('$nombre', '$checksum');"
    aplicadas=$((aplicadas + 1))
  elif [ "$registrado" != "$checksum" ]; then
    echo "migraciones: $nombre cambió después de aplicarse (drift)" >&2
    exit 1
  fi
done

echo "migraciones: OK ($aplicadas aplicadas)"
