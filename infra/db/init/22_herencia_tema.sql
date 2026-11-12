-- Herencia de tema (ADR-42). Empresa y plataforma guardan overrides parciales;
-- la tienda resuelve tienda → empresa → plataforma → defaults.
BEGIN;

ALTER TABLE identity.empresas
  ADD COLUMN IF NOT EXISTS plantilla_override text
    CHECK (plantilla_override IN ('clasica', 'minimal', 'boutique', 'mercado')),
  ADD COLUMN IF NOT EXISTS tema_override jsonb NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE identity.configuracion_plataforma
  ADD COLUMN IF NOT EXISTS plantilla_override text
    CHECK (plantilla_override IN ('clasica', 'minimal', 'boutique', 'mercado')),
  ADD COLUMN IF NOT EXISTS tema_override jsonb NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE stores.tiendas
  ADD COLUMN IF NOT EXISTS plantilla_override text
    CHECK (plantilla_override IN ('clasica', 'minimal', 'boutique', 'mercado')),
  ADD COLUMN IF NOT EXISTS tema_override jsonb NOT NULL DEFAULT '{}'::jsonb;

-- Conserva la personalización de la Fase 1 como overrides explícitos.
UPDATE stores.tiendas
SET plantilla_override = plantilla,
    tema_override = tema
WHERE personalizacion_actualizada_en IS NOT NULL
  AND plantilla_override IS NULL
  AND tema_override = '{}'::jsonb;

COMMIT;
