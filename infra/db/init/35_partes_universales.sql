-- Plataforma multi-vertical · núcleo universal de partes (fase 3).
-- Una parte representa una persona u organización que puede ser cliente,
-- proveedor, paciente, contacto, socio o cualquier otra relación del tenant.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.partes (
  id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id            uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  tipo                  text NOT NULL CHECK (tipo IN ('persona', 'organizacion')),
  nombre                text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 1 AND 240),
  nombre_comercial      text NOT NULL DEFAULT '',
  identificador_tipo    text NOT NULL DEFAULT '',
  identificador_valor   text NOT NULL DEFAULT '',
  estado                text NOT NULL DEFAULT 'activo' CHECK (estado IN ('activo', 'inactivo', 'archivado')),
  metadata              jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  creado_por            uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en             timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en        timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en          timestamptz
);

CREATE INDEX IF NOT EXISTS idx_partes_empresa_estado
  ON identity.partes (empresa_id, estado, actualizado_en DESC)
  WHERE eliminado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_partes_empresa_nombre
  ON identity.partes (empresa_id, lower(nombre))
  WHERE eliminado_en IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_partes_identificador
  ON identity.partes (empresa_id, lower(identificador_tipo), lower(identificador_valor))
  WHERE eliminado_en IS NULL AND identificador_tipo <> '' AND identificador_valor <> '';

CREATE TABLE IF NOT EXISTS identity.medios_contacto (
  id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id            uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  parte_id              uuid NOT NULL REFERENCES identity.partes(id) ON DELETE CASCADE,
  tipo                  text NOT NULL CHECK (tipo IN ('email', 'telefono', 'whatsapp', 'web', 'otro')),
  valor                 text NOT NULL CHECK (char_length(trim(valor)) BETWEEN 1 AND 240),
  etiqueta              text NOT NULL DEFAULT '',
  es_principal          boolean NOT NULL DEFAULT false,
  verificado_en         timestamptz,
  metadata              jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  creado_en             timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en        timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en          timestamptz
);

CREATE INDEX IF NOT EXISTS idx_medios_contacto_parte
  ON identity.medios_contacto (empresa_id, parte_id, tipo)
  WHERE eliminado_en IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_medios_contacto_valor
  ON identity.medios_contacto (empresa_id, parte_id, tipo, lower(valor))
  WHERE eliminado_en IS NULL;

CREATE TABLE IF NOT EXISTS identity.direcciones_partes (
  id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id            uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  parte_id              uuid NOT NULL REFERENCES identity.partes(id) ON DELETE CASCADE,
  tipo                  text NOT NULL CHECK (tipo IN ('fiscal', 'comercial', 'entrega', 'operativa', 'otro')),
  etiqueta              text NOT NULL DEFAULT '',
  linea_1               text NOT NULL CHECK (char_length(trim(linea_1)) BETWEEN 1 AND 240),
  linea_2               text NOT NULL DEFAULT '',
  ciudad                text NOT NULL DEFAULT '',
  departamento          text NOT NULL DEFAULT '',
  pais                  text NOT NULL DEFAULT '',
  codigo_postal         text NOT NULL DEFAULT '',
  referencias           text NOT NULL DEFAULT '',
  latitud               double precision CHECK (latitud BETWEEN -90 AND 90),
  longitud              double precision CHECK (longitud BETWEEN -180 AND 180),
  es_principal          boolean NOT NULL DEFAULT false,
  metadata              jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  creado_en             timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en        timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en          timestamptz
);

CREATE INDEX IF NOT EXISTS idx_direcciones_parte
  ON identity.direcciones_partes (empresa_id, parte_id, tipo)
  WHERE eliminado_en IS NULL;

CREATE TABLE IF NOT EXISTS identity.relaciones_partes (
  id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id            uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  parte_origen_id       uuid NOT NULL REFERENCES identity.partes(id) ON DELETE CASCADE,
  parte_destino_id      uuid NOT NULL REFERENCES identity.partes(id) ON DELETE CASCADE,
  tipo                  text NOT NULL CHECK (tipo ~ '^[a-z][a-z0-9_]{1,39}$'),
  metadata              jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  activo                boolean NOT NULL DEFAULT true,
  creado_por            uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en             timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en        timestamptz NOT NULL DEFAULT NOW(),
  CHECK (parte_origen_id <> parte_destino_id),
  UNIQUE (empresa_id, parte_origen_id, parte_destino_id, tipo)
);

CREATE INDEX IF NOT EXISTS idx_relaciones_partes_origen
  ON identity.relaciones_partes (empresa_id, parte_origen_id, tipo)
  WHERE activo;
CREATE INDEX IF NOT EXISTS idx_relaciones_partes_destino
  ON identity.relaciones_partes (empresa_id, parte_destino_id, tipo)
  WHERE activo;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.partes TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.medios_contacto TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.direcciones_partes TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.relaciones_partes TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.partes ENABLE ROW LEVEL SECURITY;
ALTER TABLE identity.medios_contacto ENABLE ROW LEVEL SECURITY;
ALTER TABLE identity.direcciones_partes ENABLE ROW LEVEL SECURITY;
ALTER TABLE identity.relaciones_partes ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS partes_admin ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_lectura ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_escritura ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_insert ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_update ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_delete ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_archivar ON identity.partes;
CREATE POLICY partes_admin ON identity.partes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY partes_empresa_lectura ON identity.partes FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_empresa_insert ON identity.partes FOR INSERT TO authenticated
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_empresa_update ON identity.partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_empresa_delete ON identity.partes FOR DELETE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_empresa_archivar ON identity.partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NULL)
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id')
         AND eliminado_en IS NOT NULL AND estado = 'archivado');

DROP POLICY IF EXISTS medios_contacto_admin ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_lectura ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_escritura ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_insert ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_update ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_delete ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_archivar ON identity.medios_contacto;
CREATE POLICY medios_contacto_admin ON identity.medios_contacto FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY medios_contacto_empresa_lectura ON identity.medios_contacto FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY medios_contacto_empresa_insert ON identity.medios_contacto FOR INSERT TO authenticated
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY medios_contacto_empresa_update ON identity.medios_contacto FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY medios_contacto_empresa_delete ON identity.medios_contacto FOR DELETE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY medios_contacto_empresa_archivar ON identity.medios_contacto FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NULL)
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NOT NULL);

DROP POLICY IF EXISTS direcciones_partes_admin ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_lectura ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_escritura ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_insert ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_update ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_delete ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_archivar ON identity.direcciones_partes;
CREATE POLICY direcciones_partes_admin ON identity.direcciones_partes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY direcciones_partes_empresa_lectura ON identity.direcciones_partes FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY direcciones_partes_empresa_insert ON identity.direcciones_partes FOR INSERT TO authenticated
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY direcciones_partes_empresa_update ON identity.direcciones_partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY direcciones_partes_empresa_delete ON identity.direcciones_partes FOR DELETE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY direcciones_partes_empresa_archivar ON identity.direcciones_partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NULL)
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NOT NULL);

DROP POLICY IF EXISTS relaciones_partes_admin ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_lectura ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_escritura ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_insert ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_update ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_delete ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_desactivar ON identity.relaciones_partes;
CREATE POLICY relaciones_partes_admin ON identity.relaciones_partes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY relaciones_partes_empresa_lectura ON identity.relaciones_partes FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY relaciones_partes_empresa_insert ON identity.relaciones_partes FOR INSERT TO authenticated
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY relaciones_partes_empresa_update ON identity.relaciones_partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY relaciones_partes_empresa_delete ON identity.relaciones_partes FOR DELETE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY relaciones_partes_empresa_desactivar ON identity.relaciones_partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND activo)
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND NOT activo);

COMMIT;
