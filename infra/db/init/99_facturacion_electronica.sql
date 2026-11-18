-- Facturación electrónica y PDF configurables por negocio.
--
-- La capacidad `facturacion_electronica` habilita la emisión al proveedor y la
-- descarga PDF. El proveedor y el ambiente se configuran por empresa/negocio;
-- sin la capacidad, el comprobante sigue siendo interno (comportamiento actual).
BEGIN;

ALTER TABLE finance.configuracion_fiscal
  ADD COLUMN IF NOT EXISTS facturacion_electronica boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS formato_pdf boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS proveedor_electronico text NOT NULL DEFAULT 'simulado',
  ADD COLUMN IF NOT EXISTS ambiente_electronico text NOT NULL DEFAULT 'pruebas';

ALTER TABLE finance.comprobantes
  ADD COLUMN IF NOT EXISTS empresa_id uuid,
  ADD COLUMN IF NOT EXISTS electronica_estado text,
  ADD COLUMN IF NOT EXISTS electronica_referencia text,
  ADD COLUMN IF NOT EXISTS electronica_enviada_en timestamptz;

INSERT INTO identity.capacidades (codigo, nombre, modulo)
VALUES (
  'facturacion_electronica',
  'Facturación electrónica y PDF',
  'finanzas'
)
ON CONFLICT (codigo) DO UPDATE SET nombre = EXCLUDED.nombre, modulo = EXCLUDED.modulo;

INSERT INTO identity.vertical_capacidades
  (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'facturacion_electronica', '{}'::jsonb, true
  FROM identity.verticales v
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.capacidades TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.vertical_capacidades TO core_engine_app;
  END IF;
END $$;

COMMIT;
