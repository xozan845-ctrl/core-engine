-- Fase 1 de eliminación de compatibilidad: retira proyecciones legacy de orders.
-- Destructiva a propósito: el proyecto aún no está en producción.
BEGIN;

-- 1) Proyección previa al event sourcing: ningún servicio la usa.
DROP TABLE IF EXISTS orders.proyeccion_ordenes CASCADE;

-- 2) Agregado de comisiones legacy: commissions-service es la única autoridad.
--    Primero se elimina la lógica de la función que la escribía.
CREATE OR REPLACE FUNCTION orders.fn_aplicar_evento(
  p_event_id uuid, p_aggregate_id uuid, p_tipo text, p_payload jsonb, p_version int, p_creado_en timestamptz
) RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  v_vendedor uuid; v_comprador uuid; v_tienda uuid;
  v_total int; v_comision int; v_moneda text;
  v_estado text; v_items jsonb; v_direccion jsonb;
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
        WHEN 'cancelada' THEN
          UPDATE orders.orden_vista SET estado = 'cancelada', cancelada_en = p_creado_en, actualizado_en = NOW() WHERE id = p_aggregate_id;
        WHEN 'devuelta' THEN
          UPDATE orders.orden_vista SET estado = 'devuelta', devuelta_en = p_creado_en, actualizado_en = NOW() WHERE id = p_aggregate_id;
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

DROP TABLE IF EXISTS orders.comisiones_vista CASCADE;

-- 3) Replay sin comisiones_vista, conservando la referencia universal de partes.
CREATE OR REPLACE FUNCTION orders.fn_bootstrap_proyecciones()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  r RECORD;
BEGIN
  TRUNCATE orders.orden_vista, orders.orden_timeline;
  FOR r IN SELECT * FROM orders.eventos ORDER BY aggregate_id, version LOOP
    PERFORM orders.fn_aplicar_evento(r.event_id, r.aggregate_id, r.tipo, r.payload, r.version, r.creado_en);
    IF r.tipo = 'OrderCreatedEvent'
       AND r.payload ? 'cliente_parte_id'
       AND (r.payload->>'cliente_parte_id') ~* '^[0-9a-f-]{36}$' THEN
      UPDATE orders.orden_vista
         SET parte_id = (r.payload->>'cliente_parte_id')::uuid,
             actualizado_en = NOW()
       WHERE id = r.aggregate_id;
    END IF;
  END LOOP;
END;
$$;

COMMIT;
