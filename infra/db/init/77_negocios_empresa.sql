-- ADR-0019: negocio como unidad operativa dentro de la empresa.
--
-- La empresa sigue siendo la identidad fiscal y el limite de RLS. El negocio
-- aporta la vertical, el pack y el alcance de roles, personal e invitaciones.
-- Aditivo: cada empresa existente recibe un negocio `principal` y conserva sus
-- roles y capacidades actuales sin cambios de comportamiento.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.negocios (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  codigo             text NOT NULL CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre             text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  vertical_id        text NOT NULL REFERENCES identity.verticales(codigo),
  pack_codigo        text,
  pack_version       integer,
  estado             text NOT NULL DEFAULT 'activo'
                     CHECK (estado IN ('activo', 'suspendido', 'inactivo')),
  es_predeterminado  boolean NOT NULL DEFAULT false,
  creado_por         uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, codigo),
  CONSTRAINT negocios_pack_completo_check
    CHECK ((pack_codigo IS NULL AND pack_version IS NULL)
        OR (pack_codigo IS NOT NULL AND pack_version IS NOT NULL)),
  CONSTRAINT negocios_pack_fk
    FOREIGN KEY (pack_codigo, pack_version)
    REFERENCES identity.packs(codigo, version)
);
CREATE INDEX IF NOT EXISTS idx_negocios_empresa
  ON identity.negocios (empresa_id, estado, codigo);

-- Backfill: toda empresa existente opera con un negocio principal. Se conserva
-- el pack nullable para los tenants legacy que aun no lo tienen asignado.
INSERT INTO identity.negocios
  (empresa_id, codigo, nombre, vertical_id, pack_codigo, pack_version,
   es_predeterminado, creado_por, creado_en)
SELECT e.id,
       'principal',
       left(e.nombre, 120),
       COALESCE(e.vertical_id, 'retail'),
       e.pack_codigo,
       e.pack_version,
       true,
       e.creado_por,
       e.creado_en
  FROM identity.empresas e
 WHERE NOT EXISTS (
   SELECT 1 FROM identity.negocios n WHERE n.empresa_id = e.id
 );

CREATE UNIQUE INDEX IF NOT EXISTS idx_negocios_predeterminado
  ON identity.negocios (empresa_id) WHERE es_predeterminado;

-- Alcance opcional por negocio. NULL en roles/puestos significa "de toda la
-- empresa": conserva el comportamiento previo y permite roles transversales.
ALTER TABLE identity.roles_empresa
  ADD COLUMN IF NOT EXISTS negocio_id uuid REFERENCES identity.negocios(id) ON DELETE CASCADE;
ALTER TABLE identity.puestos_empresa
  ADD COLUMN IF NOT EXISTS negocio_id uuid REFERENCES identity.negocios(id) ON DELETE CASCADE;
ALTER TABLE identity.asignaciones_personal
  ADD COLUMN IF NOT EXISTS negocio_id uuid REFERENCES identity.negocios(id) ON DELETE CASCADE;
ALTER TABLE identity.invitaciones_personal
  ADD COLUMN IF NOT EXISTS negocio_id uuid REFERENCES identity.negocios(id) ON DELETE CASCADE;
ALTER TABLE stores.tiendas
  ADD COLUMN IF NOT EXISTS negocio_id uuid REFERENCES identity.negocios(id) ON DELETE CASCADE;

-- Personal e invitaciones existentes pertenecen al negocio principal. Los
-- roles y puestos quedan transversales (negocio_id NULL) para no romper APIs.
UPDATE identity.asignaciones_personal a
   SET negocio_id = n.id
  FROM identity.negocios n
 WHERE n.empresa_id = a.empresa_id
   AND n.es_predeterminado
   AND a.negocio_id IS NULL;

UPDATE identity.invitaciones_personal i
   SET negocio_id = n.id
  FROM identity.negocios n
 WHERE n.empresa_id = i.empresa_id
   AND n.es_predeterminado
   AND i.negocio_id IS NULL;

UPDATE stores.tiendas t
   SET negocio_id = n.id
  FROM identity.negocios n
 WHERE n.empresa_id = t.empresa_id
   AND n.es_predeterminado
   AND t.negocio_id IS NULL;

-- Un codigo de rol/puesto puede repetirse en negocios distintos, pero sigue
-- siendo unico cuando es transversal a la empresa.
ALTER TABLE identity.roles_empresa
  DROP CONSTRAINT IF EXISTS roles_empresa_empresa_id_codigo_key;
CREATE UNIQUE INDEX IF NOT EXISTS idx_roles_empresa_codigo_global
  ON identity.roles_empresa (empresa_id, codigo) WHERE negocio_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_roles_empresa_codigo_negocio
  ON identity.roles_empresa (negocio_id, codigo) WHERE negocio_id IS NOT NULL;

ALTER TABLE identity.puestos_empresa
  DROP CONSTRAINT IF EXISTS puestos_empresa_empresa_id_codigo_key;
CREATE UNIQUE INDEX IF NOT EXISTS idx_puestos_empresa_codigo_global
  ON identity.puestos_empresa (empresa_id, codigo) WHERE negocio_id IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_puestos_empresa_codigo_negocio
  ON identity.puestos_empresa (negocio_id, codigo) WHERE negocio_id IS NOT NULL;

-- La misma persona puede estar invitada a dos negocios de la misma empresa.
DROP INDEX IF EXISTS identity.idx_invitaciones_personal_pendiente;
CREATE UNIQUE INDEX idx_invitaciones_personal_pendiente
  ON identity.invitaciones_personal (empresa_id, LOWER(correo), rol_empresa_id,
    COALESCE(negocio_id, '00000000-0000-0000-0000-000000000000'::uuid),
    COALESCE(tienda_id, '00000000-0000-0000-0000-000000000000'::uuid),
    COALESCE(puesto_id, '00000000-0000-0000-0000-000000000000'::uuid))
  WHERE estado = 'pendiente';

CREATE INDEX IF NOT EXISTS idx_roles_empresa_negocio
  ON identity.roles_empresa (negocio_id, activo) WHERE negocio_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_puestos_empresa_negocio
  ON identity.puestos_empresa (negocio_id, activo) WHERE negocio_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_asignaciones_personal_negocio
  ON identity.asignaciones_personal (empresa_id, negocio_id, estado);
CREATE INDEX IF NOT EXISTS idx_invitaciones_personal_negocio
  ON identity.invitaciones_personal (empresa_id, negocio_id, estado);
CREATE INDEX IF NOT EXISTS idx_tiendas_negocio
  ON stores.tiendas (negocio_id) WHERE negocio_id IS NOT NULL;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.negocios TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.negocios ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS negocios_admin ON identity.negocios;
DROP POLICY IF EXISTS negocios_tenant ON identity.negocios;
DROP POLICY IF EXISTS negocios_app ON identity.negocios;
CREATE POLICY negocios_admin ON identity.negocios FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY negocios_tenant ON identity.negocios FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario'
          AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY negocios_app ON identity.negocios FOR SELECT TO core_engine_app
  USING (true);

COMMIT;
