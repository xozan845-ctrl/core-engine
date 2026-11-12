-- Fase P1.1 · listas de precio y escalas por cantidad (R-DB-6, R-C-3).
-- Primitiva agnostica: cualquier item puede tener una lista de precios por
-- segmento (mayorista, VIP, convenio) y escalas por cantidad. La lista base
-- (lista_id NULL) materializa `tipo_precio = 'escala'` sin exigir una lista.
BEGIN;

CREATE EXTENSION IF NOT EXISTS btree_gist;

CREATE TABLE IF NOT EXISTS catalog.listas_precio (
  id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id       uuid NOT NULL,
  codigo           text NOT NULL CHECK (codigo ~ '^[a-z][a-z0-9_-]{1,39}$'),
  nombre           text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  descripcion      text NOT NULL DEFAULT '',
  moneda           text NOT NULL DEFAULT 'NIO' CHECK (moneda ~ '^[A-Z]{3}$'),
  segmento_codigo  text NOT NULL DEFAULT '' CHECK (segmento_codigo = '' OR segmento_codigo ~ '^[a-z][a-z0-9_-]{0,39}$'),
  predeterminada   boolean NOT NULL DEFAULT false,
  activo           boolean NOT NULL DEFAULT true,
  creado_en        timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en   timestamptz NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_listas_precio_empresa_codigo
  ON catalog.listas_precio (empresa_id, lower(codigo));
CREATE UNIQUE INDEX IF NOT EXISTS ux_listas_precio_predeterminada
  ON catalog.listas_precio (empresa_id)
  WHERE predeterminada AND activo;
CREATE UNIQUE INDEX IF NOT EXISTS ux_listas_precio_segmento
  ON catalog.listas_precio (empresa_id, segmento_codigo)
  WHERE segmento_codigo <> '' AND activo;
CREATE INDEX IF NOT EXISTS idx_listas_precio_empresa
  ON catalog.listas_precio (empresa_id, activo, nombre);

CREATE TABLE IF NOT EXISTS catalog.lista_precio_items (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  lista_id        uuid REFERENCES catalog.listas_precio(id) ON DELETE CASCADE,
  item_id         uuid NOT NULL REFERENCES catalog.items(id) ON DELETE CASCADE,
  desde_cantidad  integer NOT NULL DEFAULT 1 CHECK (desde_cantidad >= 1),
  hasta_cantidad  integer CHECK (hasta_cantidad IS NULL OR hasta_cantidad >= desde_cantidad),
  precio_cents    integer NOT NULL CHECK (precio_cents >= 0),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_lista_precio_items_lista
  ON catalog.lista_precio_items (lista_id, item_id, desde_cantidad)
  WHERE lista_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_lista_precio_items_base
  ON catalog.lista_precio_items (empresa_id, item_id, desde_cantidad)
  WHERE lista_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_lista_precio_items_lista
  ON catalog.lista_precio_items (lista_id, item_id, desde_cantidad);
CREATE INDEX IF NOT EXISTS idx_lista_precio_items_base
  ON catalog.lista_precio_items (empresa_id, item_id, desde_cantidad)
  WHERE lista_id IS NULL;

-- Una sola escala por rango. Las listas y la base se validan por separado
-- porque lista_id NULL no participa en indices unicos compuestos.
ALTER TABLE catalog.lista_precio_items
  DROP CONSTRAINT IF EXISTS lista_precio_items_sin_solape_lista;
ALTER TABLE catalog.lista_precio_items
  ADD CONSTRAINT lista_precio_items_sin_solape_lista
  EXCLUDE USING gist (
    lista_id WITH =,
    item_id WITH =,
    int4range(desde_cantidad, hasta_cantidad, '[]') WITH &&
  ) WHERE (lista_id IS NOT NULL);

ALTER TABLE catalog.lista_precio_items
  DROP CONSTRAINT IF EXISTS lista_precio_items_sin_solape_base;
ALTER TABLE catalog.lista_precio_items
  ADD CONSTRAINT lista_precio_items_sin_solape_base
  EXCLUDE USING gist (
    empresa_id WITH =,
    item_id WITH =,
    int4range(desde_cantidad, hasta_cantidad, '[]') WITH &&
  ) WHERE (lista_id IS NULL);

COMMENT ON TABLE catalog.listas_precio IS
  'Listas de precio por empresa y segmento; predeterminada = lista aplicada sin seleccion explicita.';
COMMENT ON TABLE catalog.lista_precio_items IS
  'Precio por item y rango de cantidad; lista_id NULL es la escala base que activa tipo_precio = escala.';

GRANT SELECT, INSERT, UPDATE, DELETE ON catalog.listas_precio TO core_engine_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON catalog.lista_precio_items TO core_engine_app;

ALTER TABLE catalog.listas_precio ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS listas_precio_admin ON catalog.listas_precio;
CREATE POLICY listas_precio_admin ON catalog.listas_precio
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS listas_precio_empresa ON catalog.listas_precio;
CREATE POLICY listas_precio_empresa ON catalog.listas_precio
  FOR ALL TO authenticated
  USING (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );
DROP POLICY IF EXISTS listas_precio_lectura ON catalog.listas_precio;
CREATE POLICY listas_precio_lectura ON catalog.listas_precio
  FOR SELECT TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

ALTER TABLE catalog.lista_precio_items ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS lista_precio_items_admin ON catalog.lista_precio_items;
CREATE POLICY lista_precio_items_admin ON catalog.lista_precio_items
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS lista_precio_items_empresa ON catalog.lista_precio_items;
CREATE POLICY lista_precio_items_empresa ON catalog.lista_precio_items
  FOR ALL TO authenticated
  USING (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );
DROP POLICY IF EXISTS lista_precio_items_lectura ON catalog.lista_precio_items;
CREATE POLICY lista_precio_items_lectura ON catalog.lista_precio_items
  FOR SELECT TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

COMMIT;
