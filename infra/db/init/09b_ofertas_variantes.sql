-- ADR-28: la oferta congela el contexto de la variante y su imagen principal.
-- producto_id sigue siendo la fila vendible (producto simple o variante); el
-- padre solo agrupa en la tienda pública.
BEGIN;

ALTER TABLE stores.ofertas ADD COLUMN IF NOT EXISTS producto_padre_id uuid;
ALTER TABLE stores.ofertas ADD COLUMN IF NOT EXISTS atributos jsonb NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE stores.ofertas ADD COLUMN IF NOT EXISTS imagen_objeto_id uuid;

CREATE INDEX IF NOT EXISTS idx_ofertas_padre
  ON stores.ofertas (tienda_id, producto_padre_id);

COMMIT;
