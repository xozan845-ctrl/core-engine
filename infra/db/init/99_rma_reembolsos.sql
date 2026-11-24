-- RMA con reembolso parcial y nota de crédito (ADR-0039).
BEGIN;

ALTER TABLE orders.devoluciones
  ADD COLUMN IF NOT EXISTS base_cents integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS impuesto_cents integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS reembolso_estado text NOT NULL DEFAULT 'pendiente'
    CHECK (reembolso_estado IN ('pendiente', 'reembolsado', 'parcial', 'fallido', 'no_aplica')),
  ADD COLUMN IF NOT EXISTS reembolsado_cents integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS reembolso_error text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS nota_credito_id uuid,
  ADD COLUMN IF NOT EXISTS nota_credito_estado text NOT NULL DEFAULT 'pendiente'
    CHECK (nota_credito_estado IN ('pendiente', 'emitida', 'fallida', 'no_aplica')),
  ADD COLUMN IF NOT EXISTS nota_credito_error text NOT NULL DEFAULT '';

ALTER TABLE orders.devolucion_lineas
  ADD COLUMN IF NOT EXISTS base_cents integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS impuesto_cents integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS tasa_impuesto_bps integer NOT NULL DEFAULT 0;

COMMIT;
