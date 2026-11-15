-- Fase P2.5 · mesas y comandas de preparacion.
-- Primitiva agnostica: salon, cocina, bar o cualquier estacion de preparacion.
BEGIN;

INSERT INTO identity.capacidades (codigo, nombre, modulo) VALUES
  ('mesas', 'Mesas y comandas', 'orders')
ON CONFLICT (codigo) DO NOTHING;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'mesas', '{}'::jsonb, true
  FROM identity.verticales v
 WHERE v.activo = true
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'mesas', '{}'::jsonb, true
  FROM identity.packs p
 WHERE p.activo = true
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO NOTHING;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, 'mesas', '{}'::jsonb, true
  FROM identity.empresas e
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

CREATE TABLE IF NOT EXISTS orders.mesas (
  id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id   uuid NOT NULL,
  codigo       text NOT NULL CHECK (codigo ~ '^[A-Za-z0-9_-]{1,20}$'),
  nombre       text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 1 AND 120),
  capacidad    integer NOT NULL DEFAULT 1 CHECK (capacidad BETWEEN 1 AND 999),
  zona         text NOT NULL DEFAULT '',
  estado       text NOT NULL DEFAULT 'libre'
               CHECK (estado IN ('libre', 'ocupada', 'reservada', 'inactiva')),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  CONSTRAINT mesas_empresa_codigo_unique UNIQUE (empresa_id, codigo)
);

CREATE INDEX IF NOT EXISTS idx_mesas_empresa_estado
  ON orders.mesas (empresa_id, estado, zona);

CREATE TABLE IF NOT EXISTS orders.comandas (
  id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id   uuid NOT NULL,
  order_id     uuid REFERENCES orders.orden_vista(id) ON DELETE SET NULL,
  mesa_id      uuid REFERENCES orders.mesas(id) ON DELETE SET NULL,
  estacion     text NOT NULL CHECK (char_length(trim(estacion)) BETWEEN 2 AND 80),
  estado       text NOT NULL DEFAULT 'pendiente'
               CHECK (estado IN ('pendiente', 'en_preparacion', 'lista', 'entregada', 'cancelada')),
  notas        text NOT NULL DEFAULT '',
  items_json   jsonb NOT NULL DEFAULT '[]'::jsonb CHECK (jsonb_typeof(items_json) = 'array'),
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  listo_en     timestamptz,
  entregado_en timestamptz
);

CREATE INDEX IF NOT EXISTS idx_comandas_empresa_estado
  ON orders.comandas (empresa_id, estado, creado_en);
CREATE INDEX IF NOT EXISTS idx_comandas_order
  ON orders.comandas (order_id);

COMMENT ON TABLE orders.mesas IS 'Mesas o puestos del salon con estado operativo.';
COMMENT ON TABLE orders.comandas IS 'Ticket de preparacion por estacion (cocina, bar, etc.).';

GRANT SELECT, INSERT, UPDATE, DELETE ON orders.mesas TO core_engine_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON orders.comandas TO core_engine_app;

ALTER TABLE orders.mesas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS mesas_admin ON orders.mesas;
CREATE POLICY mesas_admin ON orders.mesas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS mesas_empresa ON orders.mesas;
CREATE POLICY mesas_empresa ON orders.mesas
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS mesas_lectura ON orders.mesas;
CREATE POLICY mesas_lectura ON orders.mesas
  FOR SELECT TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE orders.comandas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS comandas_admin ON orders.comandas;
CREATE POLICY comandas_admin ON orders.comandas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS comandas_empresa ON orders.comandas;
CREATE POLICY comandas_empresa ON orders.comandas
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS comandas_lectura ON orders.comandas;
CREATE POLICY comandas_lectura ON orders.comandas
  FOR SELECT TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

COMMIT;
