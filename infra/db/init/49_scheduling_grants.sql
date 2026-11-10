-- Privilegios de la cuenta de aplicación para el esquema scheduling.
-- Se separa de 28_scheduling.sql para no alterar una migración ya registrada.
BEGIN;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA scheduling TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA scheduling TO core_engine_app;
  END IF;
END $$;

COMMIT;
