-- ADR-39: canal de venta del pedido.
-- delivery usa logistics-service (repartidores); pickup y consumo_local no
-- generan envío. Los pedidos históricos sin el campo quedan como delivery.

ALTER TABLE orders.orden_vista
  ADD COLUMN IF NOT EXISTS tipo_entrega text NOT NULL DEFAULT 'delivery';

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'orden_vista_tipo_entrega_check'
  ) THEN
    ALTER TABLE orders.orden_vista
      ADD CONSTRAINT orden_vista_tipo_entrega_check
      CHECK (tipo_entrega IN ('delivery', 'pickup', 'consumo_local'));
  END IF;
END $$;

-- La proyección orders.orden_vista se llena desde orders.eventos. Este trigger
-- toma el canal del OrderCreatedEvent; si el evento es histórico y no lo trae,
-- se mantiene el DEFAULT 'delivery' (retrocompatible).
CREATE OR REPLACE FUNCTION orders.fn_orden_vista_tipo_entrega() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  SELECT COALESCE(e.payload->>'tipo_entrega', 'delivery')
    INTO NEW.tipo_entrega
  FROM orders.eventos e
  WHERE e.aggregate_id = NEW.id AND e.tipo = 'OrderCreatedEvent'
  ORDER BY e.version DESC
  LIMIT 1;
  NEW.tipo_entrega := COALESCE(NEW.tipo_entrega, 'delivery');
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS orden_vista_tipo_entrega ON orders.orden_vista;
CREATE TRIGGER orden_vista_tipo_entrega
BEFORE INSERT ON orders.orden_vista
FOR EACH ROW EXECUTE FUNCTION orders.fn_orden_vista_tipo_entrega();
