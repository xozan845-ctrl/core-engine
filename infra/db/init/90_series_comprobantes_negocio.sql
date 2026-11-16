-- ADR-0019 iteración 10: series fiscales por negocio.
--
-- La serie puede ser transversal (negocio_id NULL) o pertenecer a un negocio.
-- La emisión prefiere la serie del negocio de la orden y cae a la transversal.
BEGIN;

ALTER TABLE finance.series_comprobantes
  ADD COLUMN IF NOT EXISTS negocio_id uuid;

CREATE INDEX IF NOT EXISTS idx_series_comprobantes_negocio
  ON finance.series_comprobantes (negocio_id, tipo, activa);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON finance.series_comprobantes TO core_engine_app;
  END IF;
END $$;

ALTER TABLE finance.series_comprobantes ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS series_comprobantes_admin ON finance.series_comprobantes;
DROP POLICY IF EXISTS series_comprobantes_tenant ON finance.series_comprobantes;
DROP POLICY IF EXISTS series_comprobantes_negocio_restrictiva ON finance.series_comprobantes;
CREATE POLICY series_comprobantes_admin ON finance.series_comprobantes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY series_comprobantes_tenant ON finance.series_comprobantes FOR SELECT TO authenticated
  USING (true);
CREATE POLICY series_comprobantes_negocio_restrictiva ON finance.series_comprobantes
  AS RESTRICTIVE FOR ALL TO authenticated
  USING (auth.negocio_actual() IS NULL
         OR negocio_id IS NULL
         OR negocio_id::text = auth.negocio_actual())
  WITH CHECK (auth.negocio_actual() IS NULL
         OR negocio_id IS NULL
         OR negocio_id::text = auth.negocio_actual());

COMMIT;
