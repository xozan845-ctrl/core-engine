-- Outbox de scheduling: el poller corre sin claims de usuario y necesita ver
-- sus propias filas. Es una tabla de servicio (sin PII de tenant), así que se
-- le da una política de app explícita; el resto de tablas conserva su policy.
BEGIN;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    DROP POLICY IF EXISTS outbox_app ON scheduling.outbox;
    CREATE POLICY outbox_app ON scheduling.outbox FOR ALL TO core_engine_app
      USING (true) WITH CHECK (true);
  END IF;
END $$;

COMMIT;
