-- Fase A · registros genéricos por parte (R-DB-6, R-AR-5).
-- Primitiva única para historia clínica, recetas, expedientes e inspecciones:
-- tipo + datos JSON + adjuntos + vínculo opcional a cita/item. El rubro decide
-- el tipo; la capacidad decide el acceso (historia_clinica o recetas).
BEGIN;

CREATE TABLE IF NOT EXISTS identity.partes_registros (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  parte_id        uuid NOT NULL REFERENCES identity.partes(id) ON DELETE CASCADE,
  tipo            text NOT NULL,
  titulo          text NOT NULL DEFAULT '',
  datos           jsonb NOT NULL DEFAULT '{}'::jsonb,
  adjuntos        jsonb NOT NULL DEFAULT '[]'::jsonb,
  cita_id         uuid,
  item_id         uuid,
  autor_id        uuid NOT NULL,
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en    timestamptz,
  CONSTRAINT partes_registros_tipo_check CHECK (tipo ~ '^[a-z][a-z0-9_]{1,39}$'),
  CONSTRAINT partes_registros_datos_check CHECK (jsonb_typeof(datos) = 'object'),
  CONSTRAINT partes_registros_adjuntos_check CHECK (jsonb_typeof(adjuntos) = 'array')
);

CREATE INDEX IF NOT EXISTS idx_partes_registros_parte
  ON identity.partes_registros (empresa_id, parte_id, creado_en DESC)
  WHERE eliminado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_partes_registros_tipo
  ON identity.partes_registros (empresa_id, tipo, creado_en DESC)
  WHERE eliminado_en IS NULL;

GRANT SELECT, INSERT, UPDATE, DELETE ON identity.partes_registros TO core_engine_app;

ALTER TABLE identity.partes_registros ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS partes_registros_admin ON identity.partes_registros;
CREATE POLICY partes_registros_admin ON identity.partes_registros
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

DROP POLICY IF EXISTS partes_registros_lectura ON identity.partes_registros;
CREATE POLICY partes_registros_lectura ON identity.partes_registros
  FOR SELECT TO authenticated
  USING ((empresa_id)::text = (auth.jwt() ->> 'tenant_id'));

DROP POLICY IF EXISTS partes_registros_insert ON identity.partes_registros;
CREATE POLICY partes_registros_insert ON identity.partes_registros
  FOR INSERT TO authenticated
  WITH CHECK (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

DROP POLICY IF EXISTS partes_registros_update ON identity.partes_registros;
CREATE POLICY partes_registros_update ON identity.partes_registros
  FOR UPDATE TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

DROP POLICY IF EXISTS partes_registros_delete ON identity.partes_registros;
CREATE POLICY partes_registros_delete ON identity.partes_registros
  FOR DELETE TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

-- Dental estrena la primitiva: historia clínica y recetas.
INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
VALUES
  ('dental', 'historia_clinica', '{}'::jsonb, true),
  ('dental', 'recetas', '{}'::jsonb, true)
ON CONFLICT (vertical_codigo, capacidad_codigo) DO UPDATE
  SET activo = EXCLUDED.activo, config = EXCLUDED.config;

INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT 'pack_' || vc.vertical_codigo, 1, vc.capacidad_codigo, vc.config, vc.activo
  FROM identity.vertical_capacidades vc
 WHERE vc.vertical_codigo = 'dental'
   AND vc.capacidad_codigo IN ('historia_clinica', 'recetas')
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO UPDATE
  SET activo = EXCLUDED.activo, config = EXCLUDED.config;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, vc.capacidad_codigo, vc.config, vc.activo
  FROM identity.empresas e
  JOIN identity.vertical_capacidades vc ON vc.vertical_codigo = e.vertical_id
 WHERE e.vertical_id = 'dental'
   AND vc.capacidad_codigo IN ('historia_clinica', 'recetas')
ON CONFLICT (empresa_id, capacidad_codigo) DO UPDATE
  SET activo = EXCLUDED.activo;

COMMIT;
