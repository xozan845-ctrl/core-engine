-- Fase 4 · fulfillments configurables.
-- Se siembran los cinco tipos vigentes; el checkout pasa a leerlos en el
-- siguiente PR. Aditivo: no altera órdenes existentes.
BEGIN;

CREATE TABLE IF NOT EXISTS orders.fulfillments (
  codigo              text PRIMARY KEY CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre              text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  requiere_direccion  boolean NOT NULL DEFAULT false,
  activo              boolean NOT NULL DEFAULT true,
  config              jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  creado_en           timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en      timestamptz NOT NULL DEFAULT NOW()
);

INSERT INTO orders.fulfillments (codigo, nombre, requiere_direccion, config) VALUES
  ('delivery', 'Delivery', true, '{"logistica":true}'::jsonb),
  ('pickup', 'Pickup', false, '{}'::jsonb),
  ('onsite', 'Onsite', false, '{}'::jsonb),
  ('digital', 'Digital', false, '{"entrega_digital":true}'::jsonb),
  ('none', 'Sin fulfillment', false, '{}'::jsonb)
ON CONFLICT (codigo) DO UPDATE
SET nombre = EXCLUDED.nombre,
    requiere_direccion = EXCLUDED.requiere_direccion,
    config = EXCLUDED.config,
    actualizado_en = NOW();

COMMIT;
