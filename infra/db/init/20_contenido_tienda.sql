-- Contenido del storefront de tienda (ADR-40): secciones, sobre nosotros,
-- políticas, redes y anuncio. Se guardan estructuras validadas; el frontend
-- decide cómo renderizarlas.
BEGIN;

ALTER TABLE stores.tiendas
  ADD COLUMN IF NOT EXISTS secciones jsonb NOT NULL DEFAULT '[]'::jsonb,
  ADD COLUMN IF NOT EXISTS sobre_nosotros text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS politicas jsonb NOT NULL DEFAULT '{}'::jsonb,
  ADD COLUMN IF NOT EXISTS redes jsonb NOT NULL DEFAULT '[]'::jsonb,
  ADD COLUMN IF NOT EXISTS anuncio_texto text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS anuncio_enlace text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS anuncio_activo boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS contenido_actualizado_en timestamptz;

COMMIT;
