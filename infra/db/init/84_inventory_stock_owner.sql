-- ADR-0018 · inventario es dueño del stock por ubicación.
--
-- Hasta ahora los triggers que mantenían `item_stock_ubicacion` seguían en
-- `catalog.items`, así que un item creado en `item.items` no nacía con stock
-- en su ubicación y los movimientos solo movían el total. Esta migración:
--
--   1. Garantiza una ubicación predeterminada para las empresas con items.
--   2. Repara el desglose para que sume exactamente `item.items.stock`.
--   3. Mueve los triggers a `item.items` escribiendo en `inventory.*`.
--
-- `item.items.stock` se conserva como proyección del total: la fuente es
-- `inventory.item_stock_ubicacion`. Los triggers respetan la marca
-- `inventory.stock_sync`, que usa `sincronizar_total` para no duplicar deltas.

BEGIN;

-- ── 1. Ubicación predeterminada ─────────────────────────────────────────────

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

-- Si la empresa tiene ubicaciones activas pero ninguna predeterminada, la más
-- antigua pasa a serlo (el trigger de inserción necesita una).
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

-- ── 2. Reparar el desglose contra el total ──────────────────────────────────

DO $$
DECLARE
  r           record;
  v_default   uuid;
  v_suma      integer;
  v_restante  integer;
  vr          record;
  v_quitar    integer;
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
    -- Sin ubicación activa el item conserva solo el total; el alta de la
    -- primera ubicación vuelve a hacer el backfill.
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

-- ── 3. Triggers sobre item.items ────────────────────────────────────────────

-- Delta del total → se aplica primero a la predeterminada (altas) y se
-- descuenta de las ubicaciones con stock (bajas).
CREATE OR REPLACE FUNCTION inventory.aplicar_delta_stock_ubicacion() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  v_delta      integer := COALESCE(NEW.stock, 0) - COALESCE(OLD.stock, 0);
  v_default    uuid;
  v_ubicacion  uuid;
  v_stock      integer;
  v_quitar     integer;
  v_restante   integer;
BEGIN
  IF v_delta = 0 OR NOT NEW.track_inventory THEN
    RETURN NEW;
  END IF;
  IF current_setting('inventory.stock_sync', true) = 'on' THEN
    RETURN NEW;
  END IF;
  -- Item sin desglose (empresa todavía sin ubicación activa): solo total.
  IF NOT EXISTS (
    SELECT 1 FROM inventory.item_stock_ubicacion WHERE item_id = NEW.id
  ) THEN
    RETURN NEW;
  END IF;

  PERFORM set_config('inventory.stock_sync', 'on', true);

  SELECT u.id INTO v_default
    FROM inventory.ubicaciones u
   WHERE u.empresa_id = NEW.empresa_id AND u.activo AND u.predeterminada
   LIMIT 1;

  IF v_delta > 0 THEN
    IF v_default IS NULL THEN
      SELECT ubicacion_id INTO v_default
        FROM inventory.item_stock_ubicacion
       WHERE item_id = NEW.id
       ORDER BY stock DESC, ubicacion_id
       LIMIT 1;
    END IF;
    INSERT INTO inventory.item_stock_ubicacion AS s
      (empresa_id, item_id, ubicacion_id, stock)
    VALUES (NEW.empresa_id, NEW.id, v_default, v_delta)
    ON CONFLICT (item_id, ubicacion_id) DO UPDATE
      SET stock = s.stock + EXCLUDED.stock,
          actualizado_en = NOW();
  ELSE
    v_restante := -v_delta;
    FOR v_ubicacion, v_stock IN
      SELECT ubicacion_id, stock
        FROM inventory.item_stock_ubicacion
       WHERE item_id = NEW.id AND stock > 0
       ORDER BY (ubicacion_id = COALESCE(v_default, ubicacion_id)) DESC,
                stock DESC,
                ubicacion_id
       FOR UPDATE
    LOOP
      EXIT WHEN v_restante <= 0;
      v_quitar := LEAST(v_stock, v_restante);
      UPDATE inventory.item_stock_ubicacion
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

  PERFORM set_config('inventory.stock_sync', 'off', true);
  RETURN NEW;
END;
$$;

-- Un item nuevo nace con su stock en la ubicación predeterminada, si existe.
CREATE OR REPLACE FUNCTION inventory.crear_stock_ubicacion_inicial() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  v_ubicacion uuid;
BEGIN
  IF NOT NEW.track_inventory OR NEW.eliminado_en IS NOT NULL THEN
    RETURN NEW;
  END IF;
  SELECT id INTO v_ubicacion
    FROM inventory.ubicaciones
   WHERE empresa_id = NEW.empresa_id AND activo AND predeterminada
   LIMIT 1;
  IF v_ubicacion IS NULL THEN
    RETURN NEW;
  END IF;
  INSERT INTO inventory.item_stock_ubicacion
    (empresa_id, item_id, ubicacion_id, stock)
  VALUES (NEW.empresa_id, NEW.id, v_ubicacion, COALESCE(NEW.stock, 0))
  ON CONFLICT (item_id, ubicacion_id) DO NOTHING;
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_item_items_delta_stock ON item.items;
CREATE TRIGGER trg_item_items_delta_stock
  AFTER UPDATE OF stock ON item.items
  FOR EACH ROW
  EXECUTE FUNCTION inventory.aplicar_delta_stock_ubicacion();

DROP TRIGGER IF EXISTS trg_item_items_stock_inicial ON item.items;
CREATE TRIGGER trg_item_items_stock_inicial
  AFTER INSERT ON item.items
  FOR EACH ROW
  EXECUTE FUNCTION inventory.crear_stock_ubicacion_inicial();

COMMIT;
