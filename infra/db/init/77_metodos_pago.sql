-- Fase 2 · métodos de pago configurables.
-- `orders.metodos_pago` es un catálogo global; `orders.pagos` registra los
-- intentos/confirmaciones por orden. No hay RLS en `pagos` (igual que
-- `orders.orders`): el acceso pasa siempre por orders-service, que aplica
-- tenant y roles en la consulta.
BEGIN;

CREATE TABLE IF NOT EXISTS orders.metodos_pago (
  codigo                text PRIMARY KEY CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre                text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  requiere_confirmacion boolean NOT NULL DEFAULT true,
  activo                boolean NOT NULL DEFAULT true,
  config                jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  creado_en             timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en        timestamptz NOT NULL DEFAULT NOW()
);

INSERT INTO orders.metodos_pago (codigo, nombre, requiere_confirmacion, config)
VALUES
  ('simulado', 'Simulado', false, '{"automatico":true}'::jsonb),
  ('credito', 'Crédito', false, '{"capacidad":"ventas_credito"}'::jsonb),
  ('efectivo', 'Efectivo', true, '{}'::jsonb),
  ('transferencia', 'Transferencia', true, '{}'::jsonb)
ON CONFLICT (codigo) DO UPDATE
SET nombre = EXCLUDED.nombre,
    requiere_confirmacion = EXCLUDED.requiere_confirmacion,
    config = EXCLUDED.config,
    actualizado_en = NOW();

CREATE TABLE IF NOT EXISTS orders.pagos (
  id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id      uuid NOT NULL,
  empresa_id    uuid,
  metodo_codigo text NOT NULL REFERENCES orders.metodos_pago(codigo),
  estado        text NOT NULL CHECK (estado IN ('pendiente', 'confirmado', 'rechazado', 'reembolsado')),
  monto_cents   integer NOT NULL CHECK (monto_cents > 0),
  moneda        text NOT NULL DEFAULT 'NIO' CHECK (moneda ~ '^[A-Z]{3}$'),
  referencia    text NOT NULL DEFAULT '',
  confirmado_por uuid,
  confirmado_en  timestamptz,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_orders_pagos_orden
  ON orders.pagos (order_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_orders_pagos_estado
  ON orders.pagos (estado, creado_en DESC);

ALTER TABLE orders.metodos_pago ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS metodos_pago_lectura ON orders.metodos_pago;
DROP POLICY IF EXISTS metodos_pago_admin ON orders.metodos_pago;
CREATE POLICY metodos_pago_lectura ON orders.metodos_pago
  FOR SELECT TO authenticated USING (activo);
CREATE POLICY metodos_pago_admin ON orders.metodos_pago
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE orders.pagos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS pagos_admin ON orders.pagos;
DROP POLICY IF EXISTS pagos_empresa ON orders.pagos;
CREATE POLICY pagos_admin ON orders.pagos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY pagos_empresa ON orders.pagos
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

COMMIT;
