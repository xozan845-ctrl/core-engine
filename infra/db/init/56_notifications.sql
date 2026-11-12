-- Notificaciones transaccionales (ADR-0003, fase 3).
--
-- Idempotencia por evento del bus: cada `event_id` se procesa una sola vez.
-- El envío de correo usa el adapter de @core/shared; no hay tablas de PII.
-- Aditivo e idempotente (R-DB-6).

BEGIN;

CREATE SCHEMA IF NOT EXISTS notifications;

CREATE TABLE IF NOT EXISTS notifications.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

-- La migración 51 ya barrió los esquemas existentes; éste se crea después y
-- necesita sus propios privilegios (R-CD-7/R-SB-5).
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA notifications TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA notifications TO core_engine_app;
    ALTER DEFAULT PRIVILEGES IN SCHEMA notifications
      GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO core_engine_app;
  END IF;
END $$;

COMMIT;
