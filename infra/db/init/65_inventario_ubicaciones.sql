-- Fase P1.2 · multi-bodega/sucursal y stock por ubicacion (R-DB-6, R-C-3).
-- `catalog.items.stock` sigue siendo el total de la empresa; las ubicaciones
-- desglosan ese total. Cuando un item tiene filas por ubicacion, los cambios
-- directos de stock (ordenes, ajustes) se aplican primero a la predeterminada.
BEGIN;

CREATE TABLE IF NOT EXISTS catalog.ubicaciones (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  codigo          text NOT NULL CHECK (codigo ~ '^[a-z][a-z0-9_-]{1,39}$'),
  nombre          text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  tipo            text NOT NULL DEFAULT 'bodega'
                  CHECK (tipo IN ('bodega', 'sucursal', 'almacen', 'otro')),
  direccion       text NOT NULL DEFAULT '',
  predeterminada  boolean NOT NULL DEFAULT false,
  activo          boolean NOT NULL DEFAULT true,
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_ubicaciones_empresa_codigo
  ON catalog.ubicaciones (empresa_id, lower(codigo));
CREATE UNIQUE INDEX IF NOT EXISTS ux_ubicaciones_predeterminada
  ON catalog.ubicaciones (empresa_id)
  WHERE predeterminada AND activo;
CREATE INDEX IF NOT EXISTS idx_ubicaciones_empresa
  ON catalog.ubicaciones (empresa_id, activo, nombre);

CREATE TABLE IF NOT EXISTS catalog.item_stock_ubicacion (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  item_id         uuid NOT NULL REFERENCES catalog.items(id) ON DELETE CASCADE,
  ubicacion_id    uuid NOT NULL REFERENCES catalog.ubicaciones(id) ON DELETE CASCADE,
  stock           integer NOT NULL DEFAULT 0 CHECK (stock >= 0),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW(),
  CONSTRAINT item_stock_ubicacion_unique UNIQUE (item_id, ubicacion_id)
);

CREATE INDEX IF NOT EXISTS idx_item_stock_ubicacion_item
  ON catalog.item_stock_ubicacion (empresa_id, item_id);
CREATE INDEX IF NOT EXISTS idx_item_stock_ubicacion_ubicacion
  ON catalog.item_stock_ubicacion (ubicacion_id, item_id);

COMMENT ON TABLE catalog.ubicaciones IS
  'Bodegas, sucursales o almacenes del tenant; una puede ser la predeterminada.';
COMMENT ON TABLE catalog.item_stock_ubicacion IS
  'Desglose del stock por item y ubicacion; el total sigue en catalog.items.stock.';

-- Cuando cambia el total del item, distribuye el delta en las ubicaciones.
-- No hace nada si el item no tiene desglose (compatibilidad legacy).
CREATE OR REPLACE FUNCTION catalog.aplicar_delta_stock_ubicacion() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  v_delta      integer := NEW.stock - OLD.stock;
  v_default    uuid;
  v_ubicacion  uuid;
  v_stock      integer;
  v_quitar     integer;
  v_restante   integer;
BEGIN
  IF v_delta = 0 THEN
    RETURN NEW;
  END IF;
  IF NOT EXISTS (SELECT 1 FROM catalog.item_stock_ubicacion WHERE item_id = NEW.id) THEN
    RETURN NEW;
  END IF;
  IF current_setting('catalog.stock_sync', true) = 'on' THEN
    RETURN NEW;
  END IF;

  PERFORM set_config('catalog.stock_sync', 'on', true);

  SELECT u.id INTO v_default
    FROM catalog.ubicaciones u
   WHERE u.empresa_id = NEW.empresa_id AND u.activo AND u.predeterminada
   LIMIT 1;

  IF v_delta > 0 THEN
    IF v_default IS NULL THEN
      SELECT ubicacion_id INTO v_default
        FROM catalog.item_stock_ubicacion
       WHERE item_id = NEW.id
       ORDER BY stock DESC, ubicacion_id
       LIMIT 1;
    END IF;
    INSERT INTO catalog.item_stock_ubicacion AS s
      (empresa_id, item_id, ubicacion_id, stock)
    VALUES (NEW.empresa_id, NEW.id, v_default, v_delta)
    ON CONFLICT (item_id, ubicacion_id) DO UPDATE
      SET stock = s.stock + EXCLUDED.stock,
          actualizado_en = NOW();
  ELSE
    v_restante := -v_delta;
    FOR v_ubicacion, v_stock IN
      SELECT ubicacion_id, stock
        FROM catalog.item_stock_ubicacion
       WHERE item_id = NEW.id AND stock > 0
       ORDER BY (ubicacion_id = COALESCE(v_default, ubicacion_id)) DESC,
                stock DESC,
                ubicacion_id
       FOR UPDATE
    LOOP
      EXIT WHEN v_restante <= 0;
      v_quitar := LEAST(v_stock, v_restante);
      UPDATE catalog.item_stock_ubicacion
         SET stock = stock - v_quitar,
             actualizado_en = NOW()
       WHERE item_id = NEW.id AND ubicacion_id = v_ubicacion;
      v_restante := v_restante - v_quitar;
    END LOOP;
    IF v_restante > 0 THEN
      RAISE EXCEPTION 'STOCK_UBICACION_INCONSISTENTE'
        USING DETAIL = 'El stock fisico del item no alcanza en sus ubicaciones.';
    END IF;
  END IF;

  PERFORM set_config('catalog.stock_sync', 'off', true);
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_items_delta_stock ON catalog.items;
CREATE TRIGGER trg_items_delta_stock
  AFTER UPDATE OF stock ON catalog.items
  FOR EACH ROW
  EXECUTE FUNCTION catalog.aplicar_delta_stock_ubicacion();

-- Un item nuevo nace con su stock en la ubicacion predeterminada, si existe.
CREATE OR REPLACE FUNCTION catalog.crear_stock_ubicacion_inicial() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  v_ubicacion uuid;
BEGIN
  IF NOT NEW.track_inventory OR NEW.eliminado_en IS NOT NULL THEN
    RETURN NEW;
  END IF;
  SELECT id INTO v_ubicacion
    FROM catalog.ubicaciones
   WHERE empresa_id = NEW.empresa_id AND activo AND predeterminada
   LIMIT 1;
  IF v_ubicacion IS NULL THEN
    RETURN NEW;
  END IF;
  INSERT INTO catalog.item_stock_ubicacion (empresa_id, item_id, ubicacion_id, stock)
  VALUES (NEW.empresa_id, NEW.id, v_ubicacion, COALESCE(NEW.stock, 0))
  ON CONFLICT (item_id, ubicacion_id) DO NOTHING;
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_items_stock_ubicacion_inicial ON catalog.items;
CREATE TRIGGER trg_items_stock_ubicacion_inicial
  AFTER INSERT ON catalog.items
  FOR EACH ROW
  EXECUTE FUNCTION catalog.crear_stock_ubicacion_inicial();

-- Backfill: una ubicacion principal por empresa con catalogo y el desglose de
-- todos los productos inventariables. La suma coincide con items.stock.
INSERT INTO catalog.ubicaciones (empresa_id, codigo, nombre, predeterminada)
SELECT DISTINCT i.empresa_id, 'principal', 'Principal', true
  FROM catalog.items i
 WHERE NOT EXISTS (
   SELECT 1 FROM catalog.ubicaciones u WHERE u.empresa_id = i.empresa_id
 )
ON CONFLICT DO NOTHING;

INSERT INTO catalog.item_stock_ubicacion (empresa_id, item_id, ubicacion_id, stock)
SELECT i.empresa_id, i.id, u.id, COALESCE(i.stock, 0)
  FROM catalog.items i
  JOIN catalog.ubicaciones u
    ON u.empresa_id = i.empresa_id AND u.predeterminada AND u.activo
 WHERE i.tipo = 'producto' AND i.track_inventory AND i.eliminado_en IS NULL
ON CONFLICT (item_id, ubicacion_id) DO NOTHING;

GRANT SELECT, INSERT, UPDATE, DELETE ON catalog.ubicaciones TO core_engine_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON catalog.item_stock_ubicacion TO core_engine_app;

ALTER TABLE catalog.ubicaciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS ubicaciones_admin ON catalog.ubicaciones;
CREATE POLICY ubicaciones_admin ON catalog.ubicaciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS ubicaciones_empresa ON catalog.ubicaciones;
CREATE POLICY ubicaciones_empresa ON catalog.ubicaciones
  FOR ALL TO authenticated
  USING (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );
DROP POLICY IF EXISTS ubicaciones_lectura ON catalog.ubicaciones;
CREATE POLICY ubicaciones_lectura ON catalog.ubicaciones
  FOR SELECT TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

ALTER TABLE catalog.item_stock_ubicacion ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_stock_ubicacion_admin ON catalog.item_stock_ubicacion;
CREATE POLICY item_stock_ubicacion_admin ON catalog.item_stock_ubicacion
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS item_stock_ubicacion_empresa ON catalog.item_stock_ubicacion;
CREATE POLICY item_stock_ubicacion_empresa ON catalog.item_stock_ubicacion
  FOR ALL TO authenticated
  USING (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );
DROP POLICY IF EXISTS item_stock_ubicacion_lectura ON catalog.item_stock_ubicacion;
CREATE POLICY item_stock_ubicacion_lectura ON catalog.item_stock_ubicacion
  FOR SELECT TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

COMMIT;
