-- 46 — La comisión de la proyección usa la tasa del evento (RN-04).
-- Reemplaza la función de 01d para no editar una migración ya registrada.
-- El payload de OrderCreatedEvent incluye `comision_cents` calculado con
-- COMMISSION_RATE; el 12% queda como fallback para eventos históricos.

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
      -- la comisión viaja en el evento (COMMISSION_RATE); el 12% queda solo
      -- como fallback para eventos antiguos sin el campo.
      v_comision := COALESCE((p_payload->>'comision_cents')::int, (v_total * 12 / 100)::int);
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
