-- Fase 3 · tipos de orden y transiciones configurables.
-- Se siembra exactamente la matriz vigente de `order_state.rs`; el código
-- pasa a leerla en el siguiente PR. Aditivo: no altera órdenes existentes.
BEGIN;

CREATE TABLE IF NOT EXISTS orders.estados_orden (
  codigo     text PRIMARY KEY CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre     text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  terminal   boolean NOT NULL DEFAULT false,
  creado_en  timestamptz NOT NULL DEFAULT NOW()
);

INSERT INTO orders.estados_orden (codigo, nombre, terminal) VALUES
  ('creada', 'Creada', false),
  ('pagada', 'Pagada', false),
  ('en_preparacion', 'En preparación', false),
  ('enviada', 'Enviada', false),
  ('aceptada', 'Aceptada', true),
  ('rechazada', 'Rechazada', true),
  ('entregada', 'Entregada', false),
  ('cancelada', 'Cancelada', true),
  ('devuelta', 'Devuelta', true)
ON CONFLICT (codigo) DO UPDATE
SET nombre = EXCLUDED.nombre,
    terminal = EXCLUDED.terminal;

CREATE TABLE IF NOT EXISTS orders.tipos_orden (
  codigo         text PRIMARY KEY CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre         text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  activo         boolean NOT NULL DEFAULT true,
  config         jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

INSERT INTO orders.tipos_orden (codigo, nombre, config) VALUES
  ('compra', 'Compra', '{"items":["producto","servicio","alquiler"]}'::jsonb),
  ('reserva', 'Reserva', '{"requiere_agenda":true}'::jsonb),
  ('cotizacion', 'Cotización', '{"requiere_aprobacion":true}'::jsonb),
  ('orden_servicio', 'Orden de servicio', '{"items":["servicio","alquiler"]}'::jsonb)
ON CONFLICT (codigo) DO UPDATE
SET nombre = EXCLUDED.nombre,
    config = EXCLUDED.config,
    actualizado_en = NOW();

CREATE TABLE IF NOT EXISTS orders.transiciones_orden (
  tipo_codigo text NOT NULL REFERENCES orders.tipos_orden(codigo) ON DELETE CASCADE,
  desde       text NOT NULL REFERENCES orders.estados_orden(codigo),
  hasta       text NOT NULL REFERENCES orders.estados_orden(codigo),
  PRIMARY KEY (tipo_codigo, desde, hasta),
  CHECK (desde <> hasta)
);

INSERT INTO orders.transiciones_orden (tipo_codigo, desde, hasta) VALUES
  ('compra', 'creada', 'pagada'),
  ('compra', 'creada', 'cancelada'),
  ('compra', 'pagada', 'en_preparacion'),
  ('compra', 'pagada', 'cancelada'),
  ('compra', 'pagada', 'devuelta'),
  ('compra', 'en_preparacion', 'enviada'),
  ('compra', 'en_preparacion', 'cancelada'),
  ('compra', 'enviada', 'entregada'),
  ('compra', 'enviada', 'devuelta'),
  ('compra', 'entregada', 'devuelta'),
  ('reserva', 'creada', 'pagada'),
  ('reserva', 'creada', 'cancelada'),
  ('reserva', 'pagada', 'entregada'),
  ('reserva', 'pagada', 'cancelada'),
  ('reserva', 'pagada', 'devuelta'),
  ('reserva', 'entregada', 'devuelta'),
  ('cotizacion', 'creada', 'enviada'),
  ('cotizacion', 'creada', 'cancelada'),
  ('cotizacion', 'enviada', 'aceptada'),
  ('cotizacion', 'enviada', 'rechazada'),
  ('cotizacion', 'enviada', 'cancelada'),
  ('orden_servicio', 'creada', 'pagada'),
  ('orden_servicio', 'creada', 'cancelada'),
  ('orden_servicio', 'pagada', 'en_preparacion'),
  ('orden_servicio', 'pagada', 'cancelada'),
  ('orden_servicio', 'pagada', 'devuelta'),
  ('orden_servicio', 'en_preparacion', 'entregada'),
  ('orden_servicio', 'en_preparacion', 'cancelada'),
  ('orden_servicio', 'entregada', 'devuelta')
ON CONFLICT (tipo_codigo, desde, hasta) DO NOTHING;

COMMIT;
