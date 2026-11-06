#!/bin/sh
# Roles y funciones de sesión para RLS (R-DB-2 / R-SB-2 / R-SB-3).
#
# - core_engine_app: rol de login de los servicios. NO es superusuario y NO
#   puede saltarse RLS.
# - authenticated: rol de sesión de usuario; las políticas de 99_rls.sql se
#   aplican a él. core_engine_app es miembro.
# - core_engine_service: rol de background/consumidores; BYPASSRLS para el
#   trabajo que no tiene usuario (outbox, consumidores, seed).
#
# El password llega por APP_DB_PASSWORD (runtime), nunca versionado.
set -e

if [ -z "$APP_DB_PASSWORD" ]; then
  echo "APP_DB_PASSWORD es obligatoria para crear core_engine_app" >&2
  exit 1
fi

psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<'EOSQL'
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
    CREATE ROLE anon NOLOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
    CREATE ROLE authenticated NOLOGIN;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    CREATE ROLE core_engine_app LOGIN NOSUPERUSER NOBYPASSRLS;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_service') THEN
    CREATE ROLE core_engine_service NOLOGIN BYPASSRLS;
  END IF;
END
$$;

GRANT anon TO core_engine_app;
GRANT authenticated TO core_engine_app;
GRANT core_engine_service TO core_engine_app;

CREATE SCHEMA IF NOT EXISTS auth;

CREATE OR REPLACE FUNCTION auth.jwt() RETURNS jsonb
  LANGUAGE sql STABLE AS $$
    SELECT COALESCE(NULLIF(current_setting('request.jwt.claims', true), ''), '{}')::jsonb
  $$;

-- Supabase devuelve uuid; las políticas comparan contra columnas uuid.
CREATE OR REPLACE FUNCTION auth.uid() RETURNS uuid
  LANGUAGE sql STABLE AS $$
    SELECT CASE
      WHEN (auth.jwt() ->> 'sub') ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
      THEN (auth.jwt() ->> 'sub')::uuid
      ELSE NULL
    END
  $$;

CREATE OR REPLACE FUNCTION auth.rol() RETURNS text
  LANGUAGE sql STABLE AS $$
    SELECT auth.jwt() ->> 'rol'
  $$;

GRANT USAGE ON SCHEMA auth TO authenticated, core_engine_app, core_engine_service;
GRANT EXECUTE ON FUNCTION auth.jwt(), auth.uid(), auth.rol()
  TO authenticated, core_engine_app, core_engine_service;
EOSQL

# El password se pasa con el quoting seguro de psql (:'app_password'). Debe ir
# por stdin: `psql -c` no sustituye variables.
psql -v ON_ERROR_STOP=1 -v app_password="$APP_DB_PASSWORD" \
  --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<'EOSQL'
ALTER ROLE core_engine_app PASSWORD :'app_password';
EOSQL
