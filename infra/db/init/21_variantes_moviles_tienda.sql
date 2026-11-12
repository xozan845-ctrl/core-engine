-- Variante móvil del fondo de tienda (ADR-41). Reutiliza el contrato de
-- media background del ADR-38: el binario vive en storage-service y stores
-- solo guarda la referencia y los metadatos de presentación.
BEGIN;

ALTER TABLE stores.tiendas
  ADD COLUMN IF NOT EXISTS background_movil_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS background_movil_media_type text
    CHECK (background_movil_media_type IN ('imagen', 'video')),
  ADD COLUMN IF NOT EXISTS background_movil_poster_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS background_movil_alt text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS background_movil_activo boolean NOT NULL DEFAULT false;

COMMIT;
