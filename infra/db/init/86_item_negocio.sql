-- ADR-0019 fase 4 (portado a ADR-0018): negocio en items.
--
-- `item.items` sustituye a `catalog.items`; el negocio separa catálogo y stock
-- por unidad operativa y los items existentes quedan en el principal.
BEGIN;

ALTER TABLE item.items
  ADD COLUMN IF NOT EXISTS negocio_id uuid;

UPDATE item.items i
   SET negocio_id = n.id
  FROM identity.negocios n
 WHERE n.empresa_id = i.empresa_id
   AND n.es_predeterminado
   AND i.negocio_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_items_negocio
  ON item.items (negocio_id, tipo, estado, creado_en DESC)
  WHERE eliminado_en IS NULL;

-- Default seguro para altas directas o legadas que no informan negocio.
CREATE OR REPLACE FUNCTION item.fn_item_negocio() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.negocio_id IS NULL THEN
    NEW.negocio_id := (
      SELECT id FROM identity.negocios
       WHERE empresa_id = NEW.empresa_id AND es_predeterminado
       LIMIT 1
    );
  END IF;
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS item_negocio_trigger ON item.items;
CREATE TRIGGER item_negocio_trigger
BEFORE INSERT ON item.items
FOR EACH ROW EXECUTE FUNCTION item.fn_item_negocio();

-- La vista conserva el contrato y añade la columna al final.
CREATE OR REPLACE VIEW item.items_vista AS
SELECT i.id AS item_id,
       i.empresa_id,
       i.tipo,
       i.codigo,
       i.nombre,
       i.descripcion,
       i.categoria,
       i.precio_cents,
       i.costo_cents,
       i.track_inventory,
       i.stock,
       i.duration_minutes,
       i.atributos,
       i.estado,
       i.creado_en,
       i.actualizado_en,
       i.negocio_id
  FROM item.items i
 WHERE i.eliminado_en IS NULL;

COMMIT;
