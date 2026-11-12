-- Fondo del panel de IA (ADR-43, extensión de ADR-38).
-- Segundo bloque de media background en la configuración singleton de
-- plataforma. Reutiliza el contrato, la validación y la derivación de URLs.
BEGIN;

ALTER TABLE identity.configuracion_plataforma
  ADD COLUMN IF NOT EXISTS panel_ai_background_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS panel_ai_background_media_type text,
  ADD COLUMN IF NOT EXISTS panel_ai_background_poster_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS panel_ai_background_alt text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS panel_ai_background_overlay smallint NOT NULL DEFAULT 35,
  ADD COLUMN IF NOT EXISTS panel_ai_background_position text NOT NULL DEFAULT 'center',
  ADD COLUMN IF NOT EXISTS panel_ai_background_activo boolean NOT NULL DEFAULT false;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'configuracion_plataforma_panel_ai_media_type_check'
  ) THEN
    ALTER TABLE identity.configuracion_plataforma
      ADD CONSTRAINT configuracion_plataforma_panel_ai_media_type_check
      CHECK (panel_ai_background_media_type IN ('imagen', 'video'));
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'configuracion_plataforma_panel_ai_overlay_check'
  ) THEN
    ALTER TABLE identity.configuracion_plataforma
      ADD CONSTRAINT configuracion_plataforma_panel_ai_overlay_check
      CHECK (panel_ai_background_overlay BETWEEN 0 AND 100);
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
    WHERE conname = 'configuracion_plataforma_panel_ai_position_check'
  ) THEN
    ALTER TABLE identity.configuracion_plataforma
      ADD CONSTRAINT configuracion_plataforma_panel_ai_position_check
      CHECK (panel_ai_background_position IN ('center', 'top', 'bottom', 'left', 'right'));
  END IF;
END $$;

COMMIT;
