-- ADR-0019 fase 3: capacidades por negocio.
--
-- `empresa_capacidades` se conserva como vista legada/union. La autoridad por
-- negocio es `negocio_capacidades`; el guard interno resuelve por `negocio_id`
-- y cae a la empresa solo cuando el negocio no tiene configuracion propia.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.negocio_capacidades (
  negocio_id        uuid NOT NULL REFERENCES identity.negocios(id) ON DELETE CASCADE,
  capacidad_codigo  text NOT NULL REFERENCES identity.capacidades(codigo),
  config            jsonb NOT NULL DEFAULT '{}'::jsonb
                    CHECK (jsonb_typeof(config) = 'object'),
  activo            boolean NOT NULL DEFAULT true,
  creado_en         timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en    timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (negocio_id, capacidad_codigo)
);
CREATE INDEX IF NOT EXISTS idx_negocio_capacidades_activo
  ON identity.negocio_capacidades (negocio_id, activo);

-- El negocio principal hereda exactamente lo que la empresa ya tenia activo,
-- incluyendo add-ons que no forman parte del pack.
INSERT INTO identity.negocio_capacidades
  (negocio_id, capacidad_codigo, config, activo, creado_en, actualizado_en)
SELECT n.id, ec.capacidad_codigo, ec.config, ec.activo, ec.creado_en, ec.actualizado_en
  FROM identity.negocios n
  JOIN identity.empresa_capacidades ec ON ec.empresa_id = n.empresa_id
 WHERE n.es_predeterminado
ON CONFLICT (negocio_id, capacidad_codigo) DO NOTHING;

-- Cualquier negocio sin configuracion propia arranca con las capacidades de su
-- pack. Cubre el negocio principal sin capacidades de empresa y los negocios
-- adicionales creados antes de esta fase.
INSERT INTO identity.negocio_capacidades
  (negocio_id, capacidad_codigo, config, activo, creado_en, actualizado_en)
SELECT n.id, pc.capacidad_codigo, pc.config, pc.activo, NOW(), NOW()
  FROM identity.negocios n
  JOIN identity.pack_capacidades pc
    ON pc.pack_codigo = n.pack_codigo AND pc.pack_version = n.pack_version
 WHERE NOT EXISTS (
   SELECT 1 FROM identity.negocio_capacidades nc WHERE nc.negocio_id = n.id
 )
ON CONFLICT (negocio_id, capacidad_codigo) DO NOTHING;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.negocio_capacidades TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.negocio_capacidades ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS negocio_capacidades_admin ON identity.negocio_capacidades;
DROP POLICY IF EXISTS negocio_capacidades_tenant ON identity.negocio_capacidades;
DROP POLICY IF EXISTS negocio_capacidades_app ON identity.negocio_capacidades;
CREATE POLICY negocio_capacidades_admin ON identity.negocio_capacidades
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY negocio_capacidades_tenant ON identity.negocio_capacidades
  FOR ALL TO authenticated
  USING (EXISTS (
    SELECT 1 FROM identity.negocios n
     WHERE n.id = negocio_id
       AND n.empresa_id::text = (auth.jwt() ->> 'tenant_id')
  ))
  WITH CHECK (auth.rol() = 'empresario' AND EXISTS (
    SELECT 1 FROM identity.negocios n
     WHERE n.id = negocio_id
       AND n.empresa_id::text = (auth.jwt() ->> 'tenant_id')
  ));
CREATE POLICY negocio_capacidades_app ON identity.negocio_capacidades
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- El servicio escribe negocios y capacidades con el rol de aplicación; las
-- policies de tenant siguen vigentes para `authenticated` y el filtro de
-- empresa se valida en el dominio.
DROP POLICY IF EXISTS negocios_app ON identity.negocios;
CREATE POLICY negocios_app ON identity.negocios
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

COMMIT;
