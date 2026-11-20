-- Entrega de reportes por webhook (ADR-0043).
BEGIN;
ALTER TABLE analytics.reporte_ejecuciones
  ADD COLUMN IF NOT EXISTS destino text,
  ADD COLUMN IF NOT EXISTS entrega_estado text NOT NULL DEFAULT 'no_aplica'
    CHECK (entrega_estado IN ('no_aplica', 'pendiente', 'entregado', 'fallido')),
  ADD COLUMN IF NOT EXISTS entrega_error text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS entregado_en timestamptz,
  ADD COLUMN IF NOT EXISTS intentos_entrega integer NOT NULL DEFAULT 0;
COMMIT;
