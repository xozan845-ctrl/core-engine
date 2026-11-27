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
# Orden de aplicación:
#   1. `init/*.sql`            — esquema y datos base.
#   2. `supabase/99_rls.sql`   — políticas RLS canónicas.
#   3. `supabase/99z_rls_hardening.sql` — endurecimiento y cobertura dinámica.
#   4. `supabase/migrations/*.sql` — cambios incrementales de RLS, en orden.
#
# Regla: una migración aplicada NO se edita. Cualquier cambio posterior va en un
# archivo nuevo de `supabase/migrations/AAAAAMMDD_NN_tema.sql`.
#
# Uso: PGHOST=postgres PGUSER=... PGPASSWORD=... PGDATABASE=... sh apply.sh
set -eu

DIR="${MIGRATIONS_DIR:-/migrations}"
INIT_DIR="${DIR}/init"
RLS_MIGRATIONS_DIR="${DIR}/supabase/migrations"
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

aplicadas=0

# Aplica un archivo si no está en el ledger; si ya está, verifica el checksum.
aplicar_con_ledger() {
  _archivo="$1"
  _nombre="$(basename "$_archivo")"
  _checksum="$(sha256sum "$_archivo" | cut -d' ' -f1)"
  _registrado="$($PSQL -At -c \
    "SELECT checksum FROM public.schema_migrations WHERE nombre = '$_nombre'")"
  if [ -n "$_registrado" ]; then
    if [ "$_registrado" != "$_checksum" ]; then
      echo "migraciones: $_nombre cambió después de aplicarse (drift)" >&2
      exit 1
    fi
    return 0
  fi
  echo "migraciones: aplicando $_nombre"
  $PSQL -q -c "SET request.jwt.claims = '{\"rol\":\"super_admin\"}'" -f "$_archivo"
  $PSQL -q -c "
    INSERT INTO public.schema_migrations (nombre, checksum)
    VALUES ('$_nombre', '$_checksum');"
  aplicadas=$((aplicadas + 1))
}

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

for archivo in "$INIT_DIR"/*.sql; do
  [ -f "$archivo" ] || continue
  aplicar_con_ledger "$archivo"
done

# RLS es obligatorio. Se aplica al final, después de todas las tablas, y luego
# un endurecimiento que elimina bypass, activa FORCE RLS y cubre tablas nuevas.
for archivo in "${DIR}/supabase/99_rls.sql" "${DIR}/supabase/99z_rls_hardening.sql"; do
  [ -f "$archivo" ] || continue
  aplicar_con_ledger "$archivo"
done

# Cambios incrementales de RLS posteriores al endurecimiento. Se aplican en
# orden lexicográfico y quedan registrados igual que el resto.
for archivo in "$RLS_MIGRATIONS_DIR"/*.sql; do
  [ -f "$archivo" ] || continue
  aplicar_con_ledger "$archivo"
done

echo "migraciones: OK ($aplicadas aplicadas)"
