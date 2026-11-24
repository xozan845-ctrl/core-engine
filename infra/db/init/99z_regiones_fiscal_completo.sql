-- Cierre del bloque fiscal multirregión (ADR-0049).
--
-- Completa lo que quedó pendiente en ADR-0026/0031/0038:
--   * catálogo de tasas por región y retenciones;
--   * series de comprobantes por empresa/región;
--   * retención en el comprobante;
--   * declaraciones por tenant.
--
-- La migración es aditiva y conserva las filas y series globales existentes.
BEGIN;

-- ── 1. Retenciones y tasas por región ─────────────────────────────────────
ALTER TABLE finance.jurisdicciones
  ADD COLUMN IF NOT EXISTS tasa_default_codigo text NOT NULL DEFAULT 'GENERAL',
  ADD COLUMN IF NOT EXISTS retencion_activa boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS tasa_retencion numeric(5,4) NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS retencion_codigo text NOT NULL DEFAULT 'IR';

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'jurisdicciones_tasa_retencion_check'
  ) THEN
    ALTER TABLE finance.jurisdicciones
      ADD CONSTRAINT jurisdicciones_tasa_retencion_check
      CHECK (tasa_retencion >= 0 AND tasa_retencion <= 1);
  END IF;
END $$;

CREATE TABLE IF NOT EXISTS finance.tasas_impuesto (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  jurisdiccion   text NOT NULL REFERENCES finance.jurisdicciones(codigo_pais) ON DELETE CASCADE,
  codigo         text NOT NULL CHECK (codigo ~ '^[A-Z][A-Z0-9_]{1,39}$'),
  nombre         text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  tasa           numeric(5,4) NOT NULL CHECK (tasa >= 0 AND tasa <= 1),
  exenta         boolean NOT NULL DEFAULT false,
  activa         boolean NOT NULL DEFAULT true,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (jurisdiccion, codigo)
);
CREATE INDEX IF NOT EXISTS idx_tasas_impuesto_region
  ON finance.tasas_impuesto (jurisdiccion, activa);

-- Cada región existente recibe GENERAL (su tasa_iva histórica) y EXENTA.
INSERT INTO finance.tasas_impuesto
  (jurisdiccion, codigo, nombre, tasa, exenta)
SELECT j.codigo_pais, 'GENERAL', 'Tasa general', j.tasa_iva, false
  FROM finance.jurisdicciones j
 WHERE NOT EXISTS (
   SELECT 1 FROM finance.tasas_impuesto t
    WHERE t.jurisdiccion = j.codigo_pais AND t.codigo = 'GENERAL'
 );

INSERT INTO finance.tasas_impuesto
  (jurisdiccion, codigo, nombre, tasa, exenta)
SELECT j.codigo_pais, 'EXENTA', 'Exento', 0, true
  FROM finance.jurisdicciones j
 WHERE NOT EXISTS (
   SELECT 1 FROM finance.tasas_impuesto t
    WHERE t.jurisdiccion = j.codigo_pais AND t.codigo = 'EXENTA'
 );

-- ── 2. Series de comprobantes por empresa y región ────────────────────────
ALTER TABLE finance.series_comprobantes
  ADD COLUMN IF NOT EXISTS empresa_id uuid;

CREATE INDEX IF NOT EXISTS idx_series_comprobantes_empresa
  ON finance.series_comprobantes (empresa_id, tipo, jurisdiccion, activa);

-- Aislamiento de series: cada tenant ve las suyas y las globales; los
-- administradores ven todas. Las políticas permisivas existentes se conservan.
ALTER TABLE finance.series_comprobantes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS series_comprobantes_empresa_restrictiva ON finance.series_comprobantes;
CREATE POLICY series_comprobantes_empresa_restrictiva ON finance.series_comprobantes
  AS RESTRICTIVE FOR ALL TO authenticated
  USING (
    empresa_id IS NULL
    OR empresa_id::text = (auth.jwt() ->> 'tenant_id')
    OR auth.rol() IN ('super_admin', 'admin')
  )
  WITH CHECK (
    empresa_id IS NULL
    OR empresa_id::text = (auth.jwt() ->> 'tenant_id')
    OR auth.rol() IN ('super_admin', 'admin')
  );

-- ── 3. Retención en el comprobante ────────────────────────────────────────
ALTER TABLE finance.comprobantes
  ADD COLUMN IF NOT EXISTS retencion_cents integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS retencion_tasa_bps integer NOT NULL DEFAULT 0;

-- ── 4. Declaraciones por tenant ───────────────────────────────────────────
ALTER TABLE finance.declaraciones
  ADD COLUMN IF NOT EXISTS empresa_id uuid;

ALTER TABLE finance.declaraciones
  DROP CONSTRAINT IF EXISTS declaraciones_jurisdiccion_tipo_periodo_inicio_key;

CREATE UNIQUE INDEX IF NOT EXISTS ux_declaraciones_tenant_periodo
  ON finance.declaraciones (
    COALESCE(empresa_id, '00000000-0000-0000-0000-000000000000'::uuid),
    jurisdiccion,
    tipo,
    periodo_inicio
  );
CREATE INDEX IF NOT EXISTS idx_declaraciones_empresa
  ON finance.declaraciones (empresa_id, periodo_inicio DESC);

ALTER TABLE finance.declaraciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS declaraciones_admin ON finance.declaraciones;
DROP POLICY IF EXISTS declaraciones_tenant ON finance.declaraciones;
CREATE POLICY declaraciones_admin ON finance.declaraciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY declaraciones_tenant ON finance.declaraciones
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- ── 5. RLS y privilegios del catálogo de tasas ────────────────────────────
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON finance.tasas_impuesto TO core_engine_app;
  END IF;
END $$;

ALTER TABLE finance.tasas_impuesto ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tasas_impuesto_lectura ON finance.tasas_impuesto;
DROP POLICY IF EXISTS tasas_impuesto_admin ON finance.tasas_impuesto;
CREATE POLICY tasas_impuesto_lectura ON finance.tasas_impuesto
  FOR SELECT TO anon, authenticated USING (activa);
CREATE POLICY tasas_impuesto_admin ON finance.tasas_impuesto
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

COMMIT;
