-- Fase B2 · método de pago en la proyección de órdenes (R-DB-6).
-- Las suites de orders pueden aplicar solo este cambio sin el esquema finance.
BEGIN;

ALTER TABLE orders.orden_vista
  ADD COLUMN IF NOT EXISTS metodo_pago text NOT NULL DEFAULT 'simulado';

CREATE OR REPLACE FUNCTION orders.fn_aplicar_evento(
  p_event_id uuid, p_aggregate_id uuid, p_tipo text, p_payload jsonb, p_version int, p_creado_en timestamptz
) RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  v_vendedor uuid; v_comprador uuid; v_tienda uuid;
  v_total int; v_comision int; v_moneda text;
  v_items jsonb; v_direccion jsonb;
  v_nuevo_estado text;
BEGIN
  CASE p_tipo
    WHEN 'OrderCreatedEvent' THEN
      v_vendedor := (p_payload->'items'->0->>'vendedor_id')::uuid;
      v_comprador := COALESCE(p_payload->>'comprador_id', p_payload->>'cliente_id')::uuid;
      v_tienda := (p_payload->'items'->0->>'tienda_id')::uuid;
      v_total := (p_payload->>'total_cents')::int;
      v_comision := COALESCE((p_payload->>'comision_cents')::int, (v_total * 12 / 100)::int);
      v_moneda := 'C$';
      v_items := COALESCE(p_payload->'items', '[]'::jsonb);
      v_direccion := NULL;
      INSERT INTO orders.orden_vista (
        id, vendedor_id, comprador_id, tienda_id, estado, total_cents,
        comision_cents, moneda, items_json, direccion_envio, creado_en,
        tipo_orden, fulfillment_config, parte_id, metodo_pago
      )
      VALUES (
        p_aggregate_id, v_vendedor, v_comprador, v_tienda, 'creada', v_total,
        v_comision, v_moneda, v_items, v_direccion, p_creado_en,
        p_payload->>'tipo_orden',
        COALESCE(p_payload->'fulfillment_config', '{}'::jsonb),
        NULLIF(p_payload->>'cliente_parte_id', '')::uuid,
        COALESCE(NULLIF(p_payload->>'metodo_pago', ''), 'simulado')
      );

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

COMMIT;
