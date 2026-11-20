-- Plantillas editables por evento/canal y teléfono de contacto (ADR-0043).
BEGIN;

CREATE TABLE IF NOT EXISTS notifications.plantillas (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id  uuid,
  evento      text NOT NULL CHECK (char_length(trim(evento)) BETWEEN 3 AND 80),
  canal       text NOT NULL CHECK (canal IN ('email', 'whatsapp', 'sms')),
  asunto      text NOT NULL DEFAULT '',
  cuerpo      text NOT NULL CHECK (char_length(trim(cuerpo)) BETWEEN 1 AND 4000),
  activo      boolean NOT NULL DEFAULT true,
  creado_en   timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_notificaciones_plantilla_scope
  ON notifications.plantillas
  (COALESCE(empresa_id, '00000000-0000-0000-0000-000000000000'::uuid), evento, canal);

CREATE INDEX IF NOT EXISTS idx_notificaciones_plantilla_evento
  ON notifications.plantillas (evento, canal, activo);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON notifications.plantillas TO core_engine_app;
  END IF;
END $$;

COMMIT;
