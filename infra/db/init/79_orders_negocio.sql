-- ADR-0019 fase 4: contexto de negocio en pedidos.
--
-- La orden puede agrupar líneas de varias empresas; `negocio_id` se rellena
-- solo cuando todas las líneas comparten el mismo negocio. El detalle por
-- línea vive en `items_json` (cada item lleva su `negocio_id`).
BEGIN;

ALTER TABLE orders.orden_vista
  ADD COLUMN IF NOT EXISTS negocio_id uuid;

CREATE INDEX IF NOT EXISTS idx_orden_vista_negocio
  ON orders.orden_vista (negocio_id, creado_en DESC)
  WHERE negocio_id IS NOT NULL;

CREATE OR REPLACE FUNCTION orders.fn_orden_vista_negocio() RETURNS TRIGGER
LANGUAGE plpgsql AS $$
BEGIN
  NEW.negocio_id := (
    SELECT CASE
             WHEN COUNT(DISTINCT COALESCE(item->>'negocio_id', '')) = 1
             THEN NULLIF(MIN(item->>'negocio_id'), '')::uuid
             ELSE NULL
           END
      FROM jsonb_array_elements(NEW.items_json) item
  );
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS orden_vista_negocio_trigger ON orders.orden_vista;
CREATE TRIGGER orden_vista_negocio_trigger
BEFORE INSERT OR UPDATE OF items_json ON orders.orden_vista
FOR EACH ROW EXECUTE FUNCTION orders.fn_orden_vista_negocio();

-- Backfill de pedidos existentes: solo si todas las líneas coinciden.
UPDATE orders.orden_vista v
   SET negocio_id = (
     SELECT CASE
              WHEN COUNT(DISTINCT COALESCE(item->>'negocio_id', '')) = 1
              THEN NULLIF(MIN(item->>'negocio_id'), '')::uuid
              ELSE NULL
            END
       FROM jsonb_array_elements(v.items_json) item
   )
 WHERE v.negocio_id IS NULL;

COMMIT;
