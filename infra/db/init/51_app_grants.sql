-- Privilegios de la cuenta de aplicación para todo el esquema.
-- El init crea objetos después de 00_roles.sh; sin este barrido, cualquier
-- vista/tabla nueva creada por una migración posterior queda sin acceso para
-- `core_engine_app` (los servicios responden 500 "permission denied").
-- RLS sigue aplicando: el rol no es superusuario ni tiene BYPASSRLS.
BEGIN;

DO $$
DECLARE
  esquema record;
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    RETURN;
  END IF;

  FOR esquema IN
    SELECT nspname
      FROM pg_namespace
     WHERE nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
       AND nspname NOT LIKE 'pg\_temp\_%'
       AND nspname NOT LIKE 'pg\_toast\_temp\_%'
  LOOP
    EXECUTE format('GRANT USAGE ON SCHEMA %I TO core_engine_app', esquema.nspname);
    EXECUTE format(
      'GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA %I TO core_engine_app',
      esquema.nspname
    );
    EXECUTE format(
      'GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA %I TO core_engine_app',
      esquema.nspname
    );
    EXECUTE format(
      'GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA %I TO core_engine_app',
      esquema.nspname
    );
    EXECUTE format(
      'ALTER DEFAULT PRIVILEGES IN SCHEMA %I GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO core_engine_app',
      esquema.nspname
    );
    EXECUTE format(
      'ALTER DEFAULT PRIVILEGES IN SCHEMA %I GRANT USAGE, SELECT ON SEQUENCES TO core_engine_app',
      esquema.nspname
    );
  END LOOP;
END $$;

COMMIT;
