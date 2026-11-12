-- Fase B2 · cuentas de crédito por parte y capacidad de venta a crédito
-- (R-DI-5, R-AR-5). Separada de la proyección de órdenes para pruebas.
BEGIN;

CREATE TABLE IF NOT EXISTS finance.credito_cuentas (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  parte_id       uuid NOT NULL REFERENCES identity.partes(id) ON DELETE CASCADE,
  limite_cents   integer NOT NULL CHECK (limite_cents >= 0),
  activo         boolean NOT NULL DEFAULT true,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, parte_id)
);
CREATE INDEX IF NOT EXISTS idx_credito_cuentas_empresa
  ON finance.credito_cuentas (empresa_id, activo);

GRANT SELECT, INSERT, UPDATE, DELETE ON finance.credito_cuentas TO core_engine_app;

ALTER TABLE finance.credito_cuentas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS credito_cuentas_admin ON finance.credito_cuentas;
CREATE POLICY credito_cuentas_admin ON finance.credito_cuentas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS credito_cuentas_lectura ON finance.credito_cuentas;
CREATE POLICY credito_cuentas_lectura ON finance.credito_cuentas
  FOR SELECT TO authenticated
  USING ((empresa_id)::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS credito_cuentas_escritura ON finance.credito_cuentas;
CREATE POLICY credito_cuentas_escritura ON finance.credito_cuentas
  FOR ALL TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() IN ('empresario', 'vendedor')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

-- Idempotencia del crédito creado por una orden.
CREATE UNIQUE INDEX IF NOT EXISTS ux_creditos_order
  ON finance.workspace_registros ((datos ->> 'orderId'))
  WHERE recurso = 'creditos'
    AND eliminado_en IS NULL
    AND datos ? 'orderId';

-- Capacidad ventas_credito para todos los packs activos.
INSERT INTO identity.capacidades (codigo, nombre, modulo)
VALUES ('ventas_credito', 'Ventas a credito', 'finance')
ON CONFLICT (codigo) DO NOTHING;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'ventas_credito', '{}'::jsonb, true
  FROM identity.verticales v
 WHERE v.activo = true
ON CONFLICT (vertical_codigo, capacidad_codigo) DO UPDATE
  SET activo = true, config = EXCLUDED.config;

INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'ventas_credito', '{}'::jsonb, true
  FROM identity.packs p
 WHERE p.activo = true
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO UPDATE
  SET activo = true, config = EXCLUDED.config;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, 'ventas_credito', '{}'::jsonb, true
  FROM identity.empresas e
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

COMMIT;
