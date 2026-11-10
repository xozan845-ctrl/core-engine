-- Plataforma multi-vertical · tipos de orden y fulfillment (fase 3).
-- Migracion aditiva: conserva el checkout existente y hace opcionales los
-- campos nuevos para consumidores que aun solo conocen compras de productos.
BEGIN;

ALTER TABLE orders.orden_vista
  ADD COLUMN IF NOT EXISTS tipo_orden text NOT NULL DEFAULT 'compra',
  ADD COLUMN IF NOT EXISTS fulfillment_config jsonb NOT NULL DEFAULT '{}'::jsonb;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'orden_vista_fulfillment_config_object_check'
  ) THEN
    ALTER TABLE orders.orden_vista ADD CONSTRAINT orden_vista_fulfillment_config_object_check
      CHECK (jsonb_typeof(fulfillment_config) = 'object');
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'orden_vista_tipo_orden_check'
  ) THEN
    ALTER TABLE orders.orden_vista ADD CONSTRAINT orden_vista_tipo_orden_check
      CHECK (tipo_orden IN ('compra', 'reserva', 'cotizacion', 'orden_servicio'));
  END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_orden_vista_tipo_estado
  ON orders.orden_vista (tipo_orden, estado, creado_en DESC);

-- El trigger legacy crea primero la proyeccion base. Este trigger aditivo
-- completa el tipo y el resumen de fulfillment desde el evento nuevo.
CREATE OR REPLACE FUNCTION orders.fn_sync_order_types() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'INSERT' AND NEW.tipo = 'OrderCreatedEvent' THEN
    UPDATE orders.orden_vista
       SET tipo_orden = COALESCE(NEW.payload->>'tipo_orden', 'compra'),
           fulfillment_config = COALESCE(NEW.payload->'fulfillment_config', '{}'::jsonb),
           actualizado_en = NOW()
     WHERE id = NEW.aggregate_id;
  END IF;
  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION orders.fn_bootstrap_proyecciones()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  r RECORD;
BEGIN
  TRUNCATE orders.orden_vista, orders.orden_timeline, orders.comisiones_vista;
  FOR r IN SELECT * FROM orders.eventos ORDER BY aggregate_id, version LOOP
    PERFORM orders.fn_aplicar_evento(r.event_id, r.aggregate_id, r.tipo, r.payload, r.version, r.creado_en);
    IF r.tipo = 'OrderCreatedEvent' THEN
      UPDATE orders.orden_vista
         SET tipo_orden = COALESCE(r.payload->>'tipo_orden', 'compra'),
             fulfillment_config = COALESCE(r.payload->'fulfillment_config', '{}'::jsonb),
             actualizado_en = NOW()
       WHERE id = r.aggregate_id;
    END IF;
  END LOOP;
END;
$$;

DROP TRIGGER IF EXISTS zz_order_types_projection ON orders.eventos;
CREATE TRIGGER zz_order_types_projection
AFTER INSERT ON orders.eventos
FOR EACH ROW EXECUTE FUNCTION orders.fn_sync_order_types();

COMMIT;
