-- Event Sourcing + CQRS para Orders (ADR-07 / ADR-08). Idempotente.
-- 1) Event store (append-only)
CREATE SCHEMA IF NOT EXISTS orders;

CREATE TABLE IF NOT EXISTS orders.eventos (
  event_id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  aggregate_id    uuid NOT NULL,          -- order_id
  aggregate_type  text NOT NULL DEFAULT 'Order',
  tipo            text NOT NULL,          -- OrderCreated, StockReserved, PaymentCompleted, Shipped, Delivered, Cancelled, Returned
  payload         jsonb NOT NULL,
  metadata        jsonb DEFAULT '{}'::jsonb,
  version         int NOT NULL,           -- optimistic concurrency
  creado_en       timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_eventos_aggregate ON orders.eventos (aggregate_id, version);
CREATE INDEX IF NOT EXISTS idx_eventos_tipo ON orders.eventos (tipo);
CREATE INDEX IF NOT EXISTS idx_eventos_creado ON orders.eventos (creado_en);

-- 2) Proyección: vista de orden (estado actual + resumen)
CREATE TABLE IF NOT EXISTS orders.orden_vista (
  id                  uuid PRIMARY KEY,
  vendedor_id         uuid NOT NULL,
  comprador_id        uuid NOT NULL,
  tienda_id           uuid NOT NULL,
  estado              text NOT NULL,          -- borrador, confirmada, pagada, enviada, entregada, cancelada, devuelta
  total_cents         integer NOT NULL DEFAULT 0,
  comision_cents      integer NOT NULL DEFAULT 0,
  moneda              text NOT NULL DEFAULT 'C$',
  items_json          jsonb NOT NULL DEFAULT '[]'::jsonb,
  direccion_envio     jsonb,
  creado_en           timestamptz NOT NULL,
  actualizado_en      timestamptz NOT NULL DEFAULT NOW(),
  pagada_en           timestamptz,
  enviada_en          timestamptz,
  entregada_en        timestamptz,
  cancelada_en        timestamptz,
  devuelta_en         timestamptz
);
CREATE INDEX IF NOT EXISTS idx_orden_vista_vendedor ON orders.orden_vista (vendedor_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_orden_vista_comprador ON orders.orden_vista (comprador_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_orden_vista_estado ON orders.orden_vista (estado);
CREATE INDEX IF NOT EXISTS idx_orden_vista_tienda ON orders.orden_vista (tienda_id);

-- 3) Proyección: timeline de eventos por orden (para UI/historial)
CREATE TABLE IF NOT EXISTS orders.orden_timeline (
  id              bigserial PRIMARY KEY,
  order_id        uuid NOT NULL,
  tipo            text NOT NULL,
  payload         jsonb NOT NULL,
  version         int NOT NULL,
  creado_en       timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_timeline_order ON orders.orden_timeline (order_id, version);

-- 4) Proyección: comisiones por vendedor/mes (para finanzas/liquidaciones)
CREATE TABLE IF NOT EXISTS orders.comisiones_vista (
  vendedor_id       uuid NOT NULL,
  periodo           date NOT NULL,               -- primer día del mes
  ordenes           integer NOT NULL DEFAULT 0,
  total_ventas_cents    integer NOT NULL DEFAULT 0,
  total_comision_cents  integer NOT NULL DEFAULT 0,
  actualizado_en    timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (vendedor_id, periodo)
);
CREATE INDEX IF NOT EXISTS idx_comisiones_periodo ON orders.comisiones_vista (periodo);

-- 5) Función helper: aplicar un evento a las proyecciones
CREATE OR REPLACE FUNCTION orders.fn_aplicar_evento(
  p_event_id uuid, p_aggregate_id uuid, p_tipo text, p_payload jsonb, p_version int, p_creado_en timestamptz
) RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  v_vendedor uuid; v_comprador uuid; v_tienda uuid;
  v_total int; v_comision int; v_moneda text;
  v_estado text; v_items jsonb; v_direccion jsonb;
  v_periodo date;
  v_nuevo_estado text;
BEGIN
  -- Mapear tipos de evento legacy a lógica de proyección
  CASE p_tipo
    WHEN 'OrderCreatedEvent' THEN
      -- payload: items[], estado, order_id, creado_en, cliente_id, total_cents, items[].vendedor_id, items[].tienda_id
      v_vendedor := (p_payload->'items'->0->>'vendedor_id')::uuid;
      v_comprador := (p_payload->>'cliente_id')::uuid;
      v_tienda := (p_payload->'items'->0->>'tienda_id')::uuid;
      v_total := (p_payload->>'total_cents')::int;
      -- comisión 12% (configurable)
      v_comision := (v_total * 12 / 100)::int;
      v_moneda := 'C$';
      v_items := p_payload->'items';
      v_direccion := NULL;
      INSERT INTO orders.orden_vista (id, vendedor_id, comprador_id, tienda_id, estado, total_cents, comision_cents, moneda, items_json, direccion_envio, creado_en)
      VALUES (p_aggregate_id, v_vendedor, v_comprador, v_tienda, 'creada', v_total, v_comision, v_moneda, v_items, v_direccion, p_creado_en);

    WHEN 'OrderStatusUpdatedEvent' THEN
      -- payload: estado, previo_estado, actor, motivo, order_id, ocurrido_en
      v_nuevo_estado := p_payload->>'estado';
      CASE v_nuevo_estado
        WHEN 'pagada' THEN
          UPDATE orders.orden_vista SET estado = 'pagada', pagada_en = p_creado_en, actualizado_en = NOW() WHERE id = p_aggregate_id;
        WHEN 'en_preparacion' THEN
          UPDATE orders.orden_vista SET estado = 'en_preparacion', actualizado_en = NOW() WHERE id = p_aggregate_id;
        WHEN 'enviada' THEN
          UPDATE orders.orden_vista SET estado = 'enviada', enviada_en = p_creado_en, actualizado_en = NOW() WHERE id = p_aggregate_id;
        WHEN 'entregada' THEN
          UPDATE orders.orden_vista SET estado = 'entregada', entregada_en = p_creado_en, actualizado_en = NOW() WHERE id = p_aggregate_id;
          -- comisiones_vista se contabiliza al entregar
          SELECT vendedor_id, total_cents, comision_cents INTO v_vendedor, v_total, v_comision
          FROM orders.orden_vista WHERE id = p_aggregate_id;
          v_periodo := date_trunc('month', p_creado_en)::date;
          INSERT INTO orders.comisiones_vista (vendedor_id, periodo, ordenes, total_ventas_cents, total_comision_cents, actualizado_en)
          VALUES (v_vendedor, v_periodo, 1, v_total, v_comision, NOW())
          ON CONFLICT (vendedor_id, periodo) DO UPDATE SET
            ordenes = orders.comisiones_vista.ordenes + 1,
            total_ventas_cents = orders.comisiones_vista.total_ventas_cents + v_total,
            total_comision_cents = orders.comisiones_vista.total_comision_cents + v_comision,
            actualizado_en = NOW();
        WHEN 'cancelada' THEN
          UPDATE orders.orden_vista SET estado = 'cancelada', cancelada_en = p_creado_en, actualizado_en = NOW() WHERE id = p_aggregate_id;
WHEN 'devuelta' THEN
            -- Solo revertir comisión si la orden estaba entregada (comisión ya ganada)
            SELECT vendedor_id, total_cents, comision_cents, estado INTO v_vendedor, v_total, v_comision, v_estado
            FROM orders.orden_vista WHERE id = p_aggregate_id;
            UPDATE orders.orden_vista SET estado = 'devuelta', devuelta_en = p_creado_en, actualizado_en = NOW() WHERE id = p_aggregate_id;
            IF v_estado = 'entregada' THEN
              v_periodo := date_trunc('month', p_creado_en)::date;
              INSERT INTO orders.comisiones_vista (vendedor_id, periodo, ordenes, total_ventas_cents, total_comision_cents, actualizado_en)
              VALUES (v_vendedor, v_periodo, -1, -v_total, -v_comision, NOW())
              ON CONFLICT (vendedor_id, periodo) DO UPDATE SET
                ordenes = orders.comisiones_vista.ordenes - 1,
                total_ventas_cents = orders.comisiones_vista.total_ventas_cents - v_total,
                total_comision_cents = orders.comisiones_vista.total_comision_cents - v_comision,
                actualizado_en = NOW();
            END IF;
        ELSE
          -- otros estados intermedios
          UPDATE orders.orden_vista SET estado = v_nuevo_estado, actualizado_en = NOW() WHERE id = p_aggregate_id;
      END CASE;

    ELSE
      -- otros eventos solo timeline
      NULL;
  END CASE;

  -- Siempre insertar en timeline
  INSERT INTO orders.orden_timeline (order_id, tipo, payload, version, creado_en)
  VALUES (p_aggregate_id, p_tipo, p_payload, p_version, p_creado_en);
END;
$$;

-- 6) Trigger: al insertar en event store, aplicar a proyecciones
CREATE OR REPLACE FUNCTION orders.fn_trigger_eventos() RETURNS TRIGGER LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    PERFORM orders.fn_aplicar_evento(
      NEW.event_id, NEW.aggregate_id, NEW.tipo, NEW.payload, NEW.version, NEW.creado_en
    );
  END IF;
  RETURN NULL;
END;
$$;

DROP TRIGGER IF EXISTS eventos_proyeccion_trigger ON orders.eventos;
CREATE TRIGGER eventos_proyeccion_trigger
AFTER INSERT ON orders.eventos
FOR EACH ROW EXECUTE FUNCTION orders.fn_trigger_eventos();

-- 7) Bootstrap: reconstruir proyecciones desde event store (idempotente)
CREATE OR REPLACE FUNCTION orders.fn_bootstrap_proyecciones()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  r RECORD;
BEGIN
  -- limpiar
  TRUNCATE orders.orden_vista, orders.orden_timeline, orders.comisiones_vista;
  -- replay en orden de version
  FOR r IN SELECT * FROM orders.eventos ORDER BY aggregate_id, version LOOP
    PERFORM orders.fn_aplicar_evento(r.event_id, r.aggregate_id, r.tipo, r.payload, r.version, r.creado_en);
  END LOOP;
END;
$$;

-- Ejecutar bootstrap si hay eventos
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM orders.eventos LIMIT 1) THEN
    PERFORM orders.fn_bootstrap_proyecciones();
  END IF;
END $$;