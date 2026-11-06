-- Grants mínimos para los roles de sesión de RLS (R-DB-2 / R-SB-2).
-- Se ejecuta después de 99_rls.sql (orden alfabético del entrypoint).
--
-- `authenticated` recibe los privilegios de tabla y las políticas RLS se
-- evalúan sobre él. `core_engine_service` es el rol de background (BYPASSRLS)
-- para consumidores/outbox/seed. `core_engine_app` solo inicia sesión y hace
-- SET ROLE; no necesita privilegios directos sobre las tablas.

DO $$
DECLARE
  esquema text;
  esquemas text[] := ARRAY[
    'identity', 'catalog', 'stores', 'orders', 'logistics',
    'commissions', 'finance', 'field', 'intelligence', 'storage',
    'public', 'auth'
  ];
BEGIN
  EXECUTE format(
    'GRANT CONNECT ON DATABASE %I TO core_engine_app, authenticated, core_engine_service',
    current_database()
  );

  FOREACH esquema IN ARRAY esquemas LOOP
    IF EXISTS (SELECT 1 FROM pg_namespace WHERE nspname = esquema) THEN
      EXECUTE format('GRANT USAGE ON SCHEMA %I TO authenticated, core_engine_service, core_engine_app', esquema);
      EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA %I TO authenticated, core_engine_service, core_engine_app', esquema);
      EXECUTE format('GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA %I TO authenticated, core_engine_service, core_engine_app', esquema);
      EXECUTE format('GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA %I TO authenticated, core_engine_service, core_engine_app', esquema);
      EXECUTE format('ALTER DEFAULT PRIVILEGES FOR ROLE core_engine IN SCHEMA %I GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO authenticated, core_engine_service, core_engine_app', esquema);
      EXECUTE format('ALTER DEFAULT PRIVILEGES FOR ROLE core_engine IN SCHEMA %I GRANT USAGE, SELECT ON SEQUENCES TO authenticated, core_engine_service, core_engine_app', esquema);
    END IF;
  END LOOP;
END
$$;
