#!/bin/sh
set -eu

# El compose de desarrollo usa un rol de aplicación separado del superusuario
# de Postgres. La contraseña llega por CORE_ENGINE_APP_PASSWORD; el fallback
# mantiene alineado el entorno documentado en .env.example.
APP_PASSWORD="${CORE_ENGINE_APP_PASSWORD:-}"
if [ -z "${APP_PASSWORD}" ] && [ -n "${DATABASE_URL:-}" ]; then
  APP_PASSWORD="${DATABASE_URL#*://}"
  APP_PASSWORD="${APP_PASSWORD#*:}"
  APP_PASSWORD="${APP_PASSWORD%%@*}"
fi
APP_PASSWORD="${APP_PASSWORD:-core_engine_dev}"

psql -v ON_ERROR_STOP=1 \
  --username "${POSTGRES_USER}" \
  --dbname "${POSTGRES_DB}" \
  --set=app_password="${APP_PASSWORD}" <<'SQL'
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    CREATE ROLE core_engine_app LOGIN;
  END IF;
END $$;
SELECT format('ALTER ROLE core_engine_app LOGIN PASSWORD %L', :'app_password') \gexec
SQL
