-- ADR-39: estado `servida` para consumo local / mesa.
-- Extiende el ciclo existente sin crear un flujo paralelo: una orden pagada o
-- en preparación puede marcarse servida; las comisiones devengan igual que al
-- entregar y una devolución posterior las revierte.

ALTER TABLE orders.orden_vista
  ADD COLUMN IF NOT EXISTS servida_en timestamptz;

ALTER TABLE orders.proyeccion_ordenes
  DROP CONSTRAINT IF EXISTS proyeccion_ordenes_estado_check;
ALTER TABLE orders.proyeccion_ordenes
  ADD CONSTRAINT proyeccion_ordenes_estado_check
  CHECK (estado IN (
    'creada', 'pagada', 'en_preparacion', 'enviada', 'entregada',
    'servida', 'cancelada', 'devuelta'));

-- Se reemplaza la función de proyección para que `servida` tenga el mismo
-- tratamiento contable que `entregada` (comisiones_vista) y para revertir la
-- comisión también cuando la devolución ocurre después de servir.
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
  CASE p_tipo
    WHEN 'OrderCreatedEvent' THEN
      v_vendedor := (p_payload->'items'->0->>'vendedor_id')::uuid;
      v_comprador := (p_payload->>'cliente_id')::uuid;
      v_tienda := (p_payload->'items'->0->>'tienda_id')::uuid;
      v_total := (p_payload->>'total_cents')::int;
      v_comision := (v_total * 12 / 100)::int;
      v_moneda := 'C$';
      v_items := p_payload->'items';
      v_direccion := NULL;
      INSERT INTO orders.orden_vista (id, vendedor_id, comprador_id, tienda_id, estado, total_cents, comision_cents, moneda, items_json, direccion_envio, creado_en)
      VALUES (p_aggregate_id, v_vendedor, v_comprador, v_tienda, 'creada', v_total, v_comision, v_moneda, v_items, v_direccion, p_creado_en);

    WHEN 'OrderStatusUpdatedEvent' THEN
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
        WHEN 'servida' THEN
          UPDATE orders.orden_vista SET estado = 'servida', servida_en = p_creado_en, actualizado_en = NOW() WHERE id = p_aggregate_id;
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
          SELECT vendedor_id, total_cents, comision_cents, estado INTO v_vendedor, v_total, v_comision, v_estado
          FROM orders.orden_vista WHERE id = p_aggregate_id;
          UPDATE orders.orden_vista SET estado = 'devuelta', devuelta_en = p_creado_en, actualizado_en = NOW() WHERE id = p_aggregate_id;
          IF v_estado IN ('entregada', 'servida') THEN
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
          UPDATE orders.orden_vista SET estado = v_nuevo_estado, actualizado_en = NOW() WHERE id = p_aggregate_id;
      END CASE;

    ELSE
      NULL;
  END CASE;

  INSERT INTO orders.orden_timeline (order_id, tipo, payload, version, creado_en)
  VALUES (p_aggregate_id, p_tipo, p_payload, p_version, p_creado_en);
END;
$$;
