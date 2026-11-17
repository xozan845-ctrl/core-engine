-- ADR-0020 · fase 3b: el precio base vive en pricing y el stock solo en
-- `inventory.item_stock_ubicacion`. Se retiran las columnas internas de
-- `item.items` (el contrato público ya no las exponía desde v1.9.0).

BEGIN;

-- ── 1. Precio base e histórico en pricing ───────────────────────────────────

CREATE TABLE IF NOT EXISTS pricing.historico_precios_base (
  id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id            uuid NOT NULL,
  item_id               uuid NOT NULL,
  precio_anterior_cents integer,
  precio_nuevo_cents    integer NOT NULL CHECK (precio_nuevo_cents >= 0),
  vigente_desde         timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_historico_precios_base_item
  ON pricing.historico_precios_base (empresa_id, item_id, vigente_desde DESC);

INSERT INTO pricing.historico_precios_base
  (id, empresa_id, item_id, precio_anterior_cents, precio_nuevo_cents, vigente_desde)
SELECT h.id, h.empresa_id, h.item_id, h.precio_anterior_cents,
       h.precio_nuevo_cents, h.vigente_desde
  FROM item.historico_precios h
 WHERE EXISTS (SELECT 1 FROM item.items i WHERE i.id = h.item_id)
ON CONFLICT (id) DO NOTHING;

-- Fila base: `lista_id NULL` + `desde_cantidad = 1`.
INSERT INTO pricing.lista_precio_items
  (id, empresa_id, lista_id, item_id, desde_cantidad, hasta_cantidad, precio_cents)
SELECT gen_random_uuid(), i.empresa_id, NULL, i.id, 1, NULL, i.precio_cents
  FROM item.items i
 WHERE NOT EXISTS (
   SELECT 1 FROM pricing.lista_precio_items p
    WHERE p.empresa_id = i.empresa_id
      AND p.item_id = i.id
      AND p.lista_id IS NULL
      AND p.desde_cantidad <= 1
 );

-- ── 2. Garantizar desglose de stock ─────────────────────────────────────────

INSERT INTO inventory.ubicaciones (empresa_id, codigo, nombre, predeterminada)
SELECT DISTINCT i.empresa_id, 'principal', 'Principal', true
  FROM item.items i
 WHERE i.track_inventory
   AND i.eliminado_en IS NULL
   AND NOT EXISTS (
     SELECT 1 FROM inventory.ubicaciones u
      WHERE u.empresa_id = i.empresa_id AND u.activo
   )
ON CONFLICT DO NOTHING;

WITH candidatas AS (
  SELECT DISTINCT ON (empresa_id) id, empresa_id
    FROM inventory.ubicaciones
   WHERE activo
   ORDER BY empresa_id, creado_en, id
)
UPDATE inventory.ubicaciones u
   SET predeterminada = true, actualizado_en = NOW()
  FROM candidatas c
 WHERE u.id = c.id
   AND NOT EXISTS (
     SELECT 1 FROM inventory.ubicaciones d
      WHERE d.empresa_id = c.empresa_id AND d.activo AND d.predeterminada
   );

DO $$
DECLARE
  r          record;
  v_default  uuid;
  v_suma     integer;
  v_restante integer;
  vr         record;
  v_quitar   integer;
BEGIN
  FOR r IN
    SELECT i.id, i.empresa_id, COALESCE(i.stock, 0) AS total
      FROM item.items i
     WHERE i.track_inventory AND i.eliminado_en IS NULL
  LOOP
    SELECT COALESCE(SUM(stock), 0)
      INTO v_suma
      FROM inventory.item_stock_ubicacion
     WHERE item_id = r.id;

    IF v_suma = r.total
       AND EXISTS (SELECT 1 FROM inventory.item_stock_ubicacion WHERE item_id = r.id) THEN
      CONTINUE;
    END IF;

    SELECT id INTO v_default
      FROM inventory.ubicaciones
     WHERE empresa_id = r.empresa_id AND activo AND predeterminada
     LIMIT 1;
    IF v_default IS NULL THEN
      SELECT id INTO v_default
        FROM inventory.ubicaciones
       WHERE empresa_id = r.empresa_id AND activo
       ORDER BY creado_en, id
       LIMIT 1;
    END IF;
    IF v_default IS NULL THEN
      CONTINUE;
    END IF;

    IF NOT EXISTS (SELECT 1 FROM inventory.item_stock_ubicacion WHERE item_id = r.id) THEN
      INSERT INTO inventory.item_stock_ubicacion
        (empresa_id, item_id, ubicacion_id, stock)
      VALUES (r.empresa_id, r.id, v_default, r.total);
      CONTINUE;
    END IF;

    IF r.total > v_suma THEN
      UPDATE inventory.item_stock_ubicacion
         SET stock = stock + (r.total - v_suma), actualizado_en = NOW()
       WHERE item_id = r.id AND ubicacion_id = v_default;
      IF NOT FOUND THEN
        INSERT INTO inventory.item_stock_ubicacion
          (empresa_id, item_id, ubicacion_id, stock)
        VALUES (r.empresa_id, r.id, v_default, r.total - v_suma);
      END IF;
    ELSIF r.total < v_suma THEN
      v_restante := v_suma - r.total;
      FOR vr IN
        SELECT ubicacion_id, stock
          FROM inventory.item_stock_ubicacion
         WHERE item_id = r.id AND stock > 0
         ORDER BY (ubicacion_id = v_default) DESC, stock DESC, ubicacion_id
         FOR UPDATE
      LOOP
        EXIT WHEN v_restante <= 0;
        v_quitar := LEAST(vr.stock, v_restante);
        UPDATE inventory.item_stock_ubicacion
           SET stock = stock - v_quitar, actualizado_en = NOW()
         WHERE item_id = r.id AND ubicacion_id = vr.ubicacion_id;
        v_restante := v_restante - v_quitar;
      END LOOP;
    END IF;
  END LOOP;
END $$;

-- ── 3. Retirar triggers/funciones de la proyección total ────────────────────

DROP TRIGGER IF EXISTS trg_item_items_delta_stock ON item.items;
DROP TRIGGER IF EXISTS trg_item_items_stock_inicial ON item.items;
DROP FUNCTION IF EXISTS inventory.aplicar_delta_stock_ubicacion();
DROP FUNCTION IF EXISTS inventory.crear_stock_ubicacion_inicial();

-- ── 4. Columnas, histórico y vista ─────────────────────────────────────────

DROP VIEW IF EXISTS item.items_vista;

ALTER TABLE item.items DROP COLUMN IF EXISTS precio_cents CASCADE;
ALTER TABLE item.items DROP COLUMN IF EXISTS stock CASCADE;

DROP TABLE IF EXISTS item.historico_precios CASCADE;

CREATE OR REPLACE VIEW item.items_vista AS
SELECT i.id AS item_id,
       i.empresa_id,
       i.tipo,
       i.codigo,
       i.nombre,
       i.descripcion,
       i.categoria,
       i.costo_cents,
       i.track_inventory,
       i.duration_minutes,
       i.atributos,
       i.estado,
       i.creado_en,
       i.actualizado_en
  FROM item.items i
 WHERE i.eliminado_en IS NULL;

-- ── 5. RLS/grants del histórico ────────────────────────────────────────────

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA pricing TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON pricing.historico_precios_base TO core_engine_app;
  END IF;
END $$;

ALTER TABLE pricing.historico_precios_base ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS historico_precios_base_admin ON pricing.historico_precios_base;
CREATE POLICY historico_precios_base_admin ON pricing.historico_precios_base
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS historico_precios_base_empresa ON pricing.historico_precios_base;
CREATE POLICY historico_precios_base_empresa ON pricing.historico_precios_base
  FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

COMMIT;
