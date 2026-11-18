-- Fiscalidad por tenant: cada empresa/negocio elige su jurisdicción.
--
-- Antes, `finance.tributacion.jurisdiccion_activa()` tomaba la primera
-- jurisdicción activa ordenada por código de país. Con dos países activos, el
-- resultado dependía del alfabeto. Esta tabla fija la jurisdicción efectiva por
-- empresa y, opcionalmente, por negocio.
BEGIN;

CREATE TABLE IF NOT EXISTS finance.configuracion_fiscal (
  empresa_id    uuid NOT NULL,
  negocio_id    uuid,
  jurisdiccion  text NOT NULL REFERENCES finance.jurisdicciones(codigo_pais),
  creado_en     timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

-- Un registro por empresa (negocio_id NULL) y uno por negocio.
CREATE UNIQUE INDEX IF NOT EXISTS ux_configuracion_fiscal_empresa
  ON finance.configuracion_fiscal (empresa_id)
  WHERE negocio_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_configuracion_fiscal_negocio
  ON finance.configuracion_fiscal (empresa_id, negocio_id)
  WHERE negocio_id IS NOT NULL;

-- Backfill conservador: las empresas existentes heredan la jurisdicción NI si
-- existe; no se inventan países ni se tocan negocios con configuración previa.
INSERT INTO finance.configuracion_fiscal (empresa_id, negocio_id, jurisdiccion)
SELECT e.id, NULL, 'NI'
  FROM identity.empresas e
 WHERE EXISTS (SELECT 1 FROM finance.jurisdicciones j WHERE j.codigo_pais = 'NI')
   AND NOT EXISTS (
     SELECT 1 FROM finance.configuracion_fiscal c
      WHERE c.empresa_id = e.id AND c.negocio_id IS NULL
   )
ON CONFLICT DO NOTHING;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON finance.configuracion_fiscal TO core_engine_app;
  END IF;
END $$;

ALTER TABLE finance.configuracion_fiscal ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS configuracion_fiscal_admin ON finance.configuracion_fiscal;
DROP POLICY IF EXISTS configuracion_fiscal_empresa ON finance.configuracion_fiscal;
DROP POLICY IF EXISTS configuracion_fiscal_app ON finance.configuracion_fiscal;
CREATE POLICY configuracion_fiscal_admin ON finance.configuracion_fiscal
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY configuracion_fiscal_empresa ON finance.configuracion_fiscal
  FOR ALL TO authenticated
  USING (
    auth.rol() = 'empresario'
    AND empresa_id::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() = 'empresario'
    AND empresa_id::text = (auth.jwt() ->> 'tenant_id')
  );
CREATE POLICY configuracion_fiscal_app ON finance.configuracion_fiscal
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

COMMIT;
