-- Pagos con proveedor externo, conciliación, reembolsos y contracargos.
--
-- `orders.pagos` sigue siendo el registro por orden; se añade el proveedor y
-- los montos devueltos. Los eventos de auditoría viven en `orders.pagos_eventos`.
-- Aditivo e idempotente (R-DB-6).
BEGIN;

ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS proveedor text NOT NULL DEFAULT 'simulado',
  ADD COLUMN IF NOT EXISTS monto_reembolsado_cents integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS conciliado_en timestamptz;

ALTER TABLE orders.pagos
  DROP CONSTRAINT IF EXISTS pagos_estado_check;
ALTER TABLE orders.pagos
  ADD CONSTRAINT pagos_estado_check
  CHECK (estado IN (
    'pendiente', 'confirmado', 'rechazado', 'reembolsado',
    'reembolsado_parcial', 'contracargo'
  ));

CREATE TABLE IF NOT EXISTS orders.pagos_eventos (
  id           bigserial PRIMARY KEY,
  pago_id      uuid NOT NULL REFERENCES orders.pagos(id) ON DELETE CASCADE,
  tipo         text NOT NULL,
  monto_cents  integer NOT NULL DEFAULT 0,
  motivo       text NOT NULL DEFAULT '',
  actor_id     uuid,
  payload      jsonb NOT NULL DEFAULT '{}'::jsonb,
  creado_en    timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_pagos_eventos_pago
  ON orders.pagos_eventos (pago_id, creado_en DESC);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON orders.pagos_eventos TO core_engine_app;
    GRANT USAGE, SELECT ON SEQUENCE orders.pagos_eventos_id_seq TO core_engine_app;
  END IF;
END $$;

COMMIT;
