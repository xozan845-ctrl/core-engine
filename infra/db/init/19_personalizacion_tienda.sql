-- Personalización visual de tienda (ADR-39). El backend guarda tokens
-- validados (plantilla, tema, hero y SEO) y el frontend los renderiza. No se
-- aceptan CSS/HTML/JS libres; el tema es JSONB con allowlist en el servicio.
BEGIN;

ALTER TABLE stores.tiendas
  ADD COLUMN IF NOT EXISTS plantilla text NOT NULL DEFAULT 'clasica'
    CHECK (plantilla IN ('clasica', 'minimal', 'boutique', 'mercado')),
  ADD COLUMN IF NOT EXISTS tema jsonb NOT NULL DEFAULT '{}'::jsonb,
  ADD COLUMN IF NOT EXISTS hero_titulo text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS hero_subtitulo text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS hero_cta_texto text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS hero_cta_url text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS seo_titulo text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS seo_descripcion text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS seo_imagen_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS personalizacion_actualizada_en timestamptz;

COMMIT;
