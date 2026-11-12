-- Plataforma multi-vertical · ofertas de cualquier item (fase 2).
-- Conserva producto_id para compatibilidad y agrega item_type/item_id.
BEGIN;

ALTER TABLE stores.ofertas
  ADD COLUMN IF NOT EXISTS item_type text NOT NULL DEFAULT 'producto',
  ADD COLUMN IF NOT EXISTS item_id uuid;

UPDATE stores.ofertas o
   SET item_id = i.id
  FROM catalog.items i
 WHERE i.source_type = 'producto'
   AND i.source_id = o.producto_id
   AND o.item_id IS NULL;

ALTER TABLE stores.ofertas
  ALTER COLUMN producto_id DROP NOT NULL,
  ALTER COLUMN stock DROP NOT NULL;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'ofertas_item_type_check'
  ) THEN
    ALTER TABLE stores.ofertas ADD CONSTRAINT ofertas_item_type_check
      CHECK (item_type IN ('producto', 'servicio', 'alquiler'));
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'ofertas_item_id_fkey'
  ) THEN
    ALTER TABLE stores.ofertas ADD CONSTRAINT ofertas_item_id_fkey
      FOREIGN KEY (item_id) REFERENCES catalog.items(id);
  END IF;
END $$;

ALTER TABLE stores.ofertas ALTER COLUMN item_id SET NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_ofertas_tienda_item
  ON stores.ofertas (tienda_id, item_type, item_id);
CREATE INDEX IF NOT EXISTS idx_ofertas_item
  ON stores.ofertas (item_type, item_id, estado);

COMMIT;
