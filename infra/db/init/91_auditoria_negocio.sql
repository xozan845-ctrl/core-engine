-- ADR-0021 iteración 11: auditoría por empresa y negocio.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.auditoria (
  id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id       uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  negocio_id       uuid REFERENCES identity.negocios(id) ON DELETE SET NULL,
  actor_usuario_id uuid REFERENCES identity.usuarios(id) ON DELETE SET NULL,
  accion           text NOT NULL CHECK (char_length(trim(accion)) BETWEEN 3 AND 120),
  recurso          text NOT NULL CHECK (char_length(trim(recurso)) BETWEEN 2 AND 80),
  recurso_id       text,
  request_id       text,
  datos            jsonb NOT NULL DEFAULT '{}'::jsonb
                   CHECK (jsonb_typeof(datos) = 'object'),
  creado_en        timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_auditoria_empresa_negocio
  ON identity.auditoria (empresa_id, negocio_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_auditoria_accion
  ON identity.auditoria (accion, creado_en DESC);

INSERT INTO identity.permisos (codigo, modulo, nombre, descripcion) VALUES
  ('auditoria.leer', 'identity', 'Leer auditoría', 'Consultar cambios de negocio, roles, personal y capacidades.')
ON CONFLICT (codigo) DO UPDATE
  SET modulo = EXCLUDED.modulo,
      nombre = EXCLUDED.nombre,
      descripcion = EXCLUDED.descripcion,
      version = identity.permisos.version + 1,
      activo = true;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.auditoria TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.auditoria ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS auditoria_admin ON identity.auditoria;
DROP POLICY IF EXISTS auditoria_tenant ON identity.auditoria;
DROP POLICY IF EXISTS auditoria_app ON identity.auditoria;
CREATE POLICY auditoria_admin ON identity.auditoria FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY auditoria_tenant ON identity.auditoria FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id')
         AND (auth.negocio_actual() IS NULL
              OR negocio_id IS NULL
              OR negocio_id::text = auth.negocio_actual()));
CREATE POLICY auditoria_app ON identity.auditoria FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

COMMIT;
