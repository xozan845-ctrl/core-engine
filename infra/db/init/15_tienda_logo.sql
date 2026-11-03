-- Logo de tienda. El binario vive en storage-service; stores solo guarda la
-- referencia al objeto público y la valida por API interna (sin FK cruzada).
BEGIN;

ALTER TABLE stores.tiendas
  ADD COLUMN IF NOT EXISTS logo_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS actualizado_en timestamptz NOT NULL DEFAULT NOW();

COMMIT;
