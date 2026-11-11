-- Fase C · tarifas por fecha/rango sobre items (R-DB-6, R-C-3).
-- Primitiva agnóstica: cualquier item puede tener precios por temporada; la
-- agenda existente aporta la disponibilidad por recurso.
BEGIN;

CREATE EXTENSION IF NOT EXISTS btree_gist;

CREATE TABLE IF NOT EXISTS catalog.item_tarifas (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  item_id        uuid NOT NULL REFERENCES catalog.items(id) ON DELETE CASCADE,
  desde          date NOT NULL,
  hasta          date NOT NULL,
  precio_cents   integer NOT NULL CHECK (precio_cents >= 0),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  CONSTRAINT item_tarifas_rango_check CHECK (hasta >= desde),
  CONSTRAINT item_tarifas_sin_solape
    EXCLUDE USING gist (item_id WITH =, daterange(desde, hasta, '[]') WITH &&)
);

CREATE INDEX IF NOT EXISTS idx_item_tarifas_item
  ON catalog.item_tarifas (empresa_id, item_id, desde);

GRANT SELECT, INSERT, UPDATE, DELETE ON catalog.item_tarifas TO core_engine_app;

ALTER TABLE catalog.item_tarifas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_tarifas_admin ON catalog.item_tarifas;
CREATE POLICY item_tarifas_admin ON catalog.item_tarifas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS item_tarifas_empresa ON catalog.item_tarifas;
CREATE POLICY item_tarifas_empresa ON catalog.item_tarifas
  FOR ALL TO authenticated
  USING (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );
DROP POLICY IF EXISTS item_tarifas_lectura ON catalog.item_tarifas;
CREATE POLICY item_tarifas_lectura ON catalog.item_tarifas
  FOR SELECT TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

COMMIT;
