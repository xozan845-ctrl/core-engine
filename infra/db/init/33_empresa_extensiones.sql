-- Plataforma multi-vertical · extensiones configurables por empresa (fase 2).
-- Las extensiones describen datos y formularios; no ejecutan workflows ni código.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.extensiones_empresa (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  codigo             text NOT NULL CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre             text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  recurso            text NOT NULL CHECK (recurso ~ '^[a-z][a-z0-9_]{1,39}$'),
  descripcion        text NOT NULL DEFAULT '' CHECK (char_length(descripcion) <= 1000),
  esquema            jsonb NOT NULL CHECK (jsonb_typeof(esquema) = 'object'),
  formulario         jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(formulario) = 'object'),
  reglas             jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(reglas) = 'object'),
  version            integer NOT NULL CHECK (version >= 1),
  activo             boolean NOT NULL DEFAULT true,
  creado_por         uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, codigo, version)
);

CREATE INDEX IF NOT EXISTS idx_extensiones_empresa_recurso
  ON identity.extensiones_empresa (empresa_id, recurso, codigo, activo);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.extensiones_empresa TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.extensiones_empresa ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS extensiones_empresa_admin ON identity.extensiones_empresa;
DROP POLICY IF EXISTS extensiones_empresa_lectura ON identity.extensiones_empresa;
DROP POLICY IF EXISTS extensiones_empresa_insert ON identity.extensiones_empresa;
DROP POLICY IF EXISTS extensiones_empresa_update ON identity.extensiones_empresa;
CREATE POLICY extensiones_empresa_admin ON identity.extensiones_empresa FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY extensiones_empresa_lectura ON identity.extensiones_empresa FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extensiones_empresa_insert ON identity.extensiones_empresa FOR INSERT TO authenticated
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extensiones_empresa_update ON identity.extensiones_empresa FOR UPDATE TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

COMMIT;
