-- Teléfono de contacto para notificaciones WhatsApp/SMS (ADR-0043).
BEGIN;
ALTER TABLE identity.usuarios
  ADD COLUMN IF NOT EXISTS telefono text;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, UPDATE ON identity.usuarios TO core_engine_app;
  END IF;
END $$;
COMMIT;
