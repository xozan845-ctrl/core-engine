-- Fase P2.4 · moneda explicita por oferta.
-- La moneda viaja con la oferta y la orden; no hay conversion implicita. Un
-- pedido no puede mezclar ofertas de monedas distintas.
BEGIN;

ALTER TABLE stores.ofertas
  ADD COLUMN IF NOT EXISTS moneda text NOT NULL DEFAULT 'NIO';

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'ofertas_moneda_check'
  ) THEN
    ALTER TABLE stores.ofertas
      ADD CONSTRAINT ofertas_moneda_check CHECK (moneda ~ '^[A-Z]{3}$');
  END IF;
END $$;

UPDATE stores.ofertas o
   SET moneda = e.moneda
  FROM stores.tiendas t
  JOIN identity.empresas e ON e.id = t.empresa_id
 WHERE o.tienda_id = t.id
   AND o.moneda = 'NIO'
   AND e.moneda <> 'NIO';

COMMENT ON COLUMN stores.ofertas.moneda IS
  'Codigo ISO de la moneda final de la oferta; se copia a la orden.';

COMMIT;
