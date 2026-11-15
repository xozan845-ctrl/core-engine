-- Fase 2 · motivo del rechazo de un pago manual.
BEGIN;
ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS motivo_rechazo text NOT NULL DEFAULT '';
COMMIT;
