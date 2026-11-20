-- Ajustes de devoluciones y cancelaciones sobre hechos analíticos (ADR-0043).
-- Los hechos originales son inmutables; los ajustes se guardan como filas
-- negativas con `tipo` explícito para que los reportes neten sin perder traza.
BEGIN;

ALTER TABLE analytics.hechos_venta
  ADD COLUMN IF NOT EXISTS tipo text NOT NULL DEFAULT 'venta'
  CHECK (tipo IN ('venta', 'devolucion', 'cancelacion'));

ALTER TABLE analytics.comisiones
  ADD COLUMN IF NOT EXISTS tipo text NOT NULL DEFAULT 'comision'
  CHECK (tipo IN ('comision', 'devolucion', 'cancelacion'));

CREATE INDEX IF NOT EXISTS idx_analytics_hechos_tipo
  ON analytics.hechos_venta (tipo, order_id);
CREATE INDEX IF NOT EXISTS idx_analytics_comisiones_tipo
  ON analytics.comisiones (tipo, order_id);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON analytics.hechos_venta TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON analytics.comisiones TO core_engine_app;
  END IF;
END $$;

COMMIT;
