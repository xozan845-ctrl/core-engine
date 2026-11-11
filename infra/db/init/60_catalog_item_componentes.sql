-- Fase D · composición de items (BOM/kits) (R-DB-6, R-C-3).
-- Un item compuesto consume sus componentes al venderlo; sirve para recetas de
-- restaurante, kits y combos sin lógica por rubro.
BEGIN;

CREATE TABLE IF NOT EXISTS catalog.item_componentes (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL,
  item_id            uuid NOT NULL REFERENCES catalog.items(id) ON DELETE CASCADE,
  componente_item_id uuid NOT NULL REFERENCES catalog.items(id) ON DELETE RESTRICT,
  cantidad           integer NOT NULL CHECK (cantidad > 0),
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW(),
  CONSTRAINT item_componentes_distintos CHECK (item_id <> componente_item_id),
  CONSTRAINT item_componentes_unique UNIQUE (item_id, componente_item_id)
);

CREATE INDEX IF NOT EXISTS idx_item_componentes_empresa
  ON catalog.item_componentes (empresa_id, item_id);

GRANT SELECT, INSERT, UPDATE, DELETE ON catalog.item_componentes TO core_engine_app;

ALTER TABLE catalog.item_componentes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_componentes_admin ON catalog.item_componentes;
CREATE POLICY item_componentes_admin ON catalog.item_componentes
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS item_componentes_empresa ON catalog.item_componentes;
CREATE POLICY item_componentes_empresa ON catalog.item_componentes
  FOR ALL TO authenticated
  USING (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );
DROP POLICY IF EXISTS item_componentes_lectura ON catalog.item_componentes;
CREATE POLICY item_componentes_lectura ON catalog.item_componentes
  FOR SELECT TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

COMMIT;
