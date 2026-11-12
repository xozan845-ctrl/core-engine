-- Plataforma multi-vertical · valores de extensiones por recurso (fase 2).
-- Cada versión de una extensión conserva sus propios valores; no se reescribe
-- historia cuando el tenant publica una nueva versión del schema.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.extensiones_empresa_valores (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  extension_id       uuid NOT NULL REFERENCES identity.extensiones_empresa(id) ON DELETE CASCADE,
  recurso_id         uuid NOT NULL,
  valores            jsonb NOT NULL CHECK (jsonb_typeof(valores) = 'object'),
  version_schema     integer NOT NULL CHECK (version_schema >= 1),
  creado_por         uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, extension_id, recurso_id)
);

CREATE INDEX IF NOT EXISTS idx_extension_values_recurso
  ON identity.extensiones_empresa_valores (empresa_id, recurso_id, extension_id);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE
      ON identity.extensiones_empresa_valores TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.extensiones_empresa_valores ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS extension_values_admin ON identity.extensiones_empresa_valores;
DROP POLICY IF EXISTS extension_values_lectura ON identity.extensiones_empresa_valores;
DROP POLICY IF EXISTS extension_values_insert ON identity.extensiones_empresa_valores;
DROP POLICY IF EXISTS extension_values_update ON identity.extensiones_empresa_valores;
DROP POLICY IF EXISTS extension_values_delete ON identity.extensiones_empresa_valores;
CREATE POLICY extension_values_admin ON identity.extensiones_empresa_valores FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY extension_values_lectura ON identity.extensiones_empresa_valores FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extension_values_insert ON identity.extensiones_empresa_valores FOR INSERT TO authenticated
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extension_values_update ON identity.extensiones_empresa_valores FOR UPDATE TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extension_values_delete ON identity.extensiones_empresa_valores FOR DELETE TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

COMMIT;
