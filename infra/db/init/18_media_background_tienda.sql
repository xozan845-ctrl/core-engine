-- Fondo de tienda (ADR-38). El binario vive en storage-service; stores solo
-- guarda la referencia al objeto publico, el tipo derivado y los metadatos de
-- presentacion (sin FK cruzada, R-DB-1).
BEGIN;

ALTER TABLE stores.tiendas
  ADD COLUMN IF NOT EXISTS background_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS background_media_type text
    CHECK (background_media_type IN ('imagen', 'video')),
  ADD COLUMN IF NOT EXISTS background_poster_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS background_alt text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS background_overlay smallint NOT NULL DEFAULT 35
    CHECK (background_overlay BETWEEN 0 AND 100),
  ADD COLUMN IF NOT EXISTS background_position text NOT NULL DEFAULT 'center'
    CHECK (background_position IN ('center', 'top', 'bottom', 'left', 'right')),
  ADD COLUMN IF NOT EXISTS background_activo boolean NOT NULL DEFAULT false;

COMMIT;
