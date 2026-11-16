-- Esquema mínimo de la migración para las pruebas de inventario (ADR-0018).
-- Esquema de pruebas del corte PR 5: `inventory` para inventario y `catalog`
-- esquema reproduce solo las tablas que el dominio necesita.
DROP SCHEMA IF EXISTS item CASCADE;
DROP SCHEMA IF EXISTS inventory CASCADE;
CREATE SCHEMA item;
CREATE SCHEMA inventory;

CREATE TABLE item.items (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  tipo            text NOT NULL DEFAULT 'producto',
  codigo          text NOT NULL,
  nombre          text NOT NULL,
  precio_cents    integer NOT NULL DEFAULT 0,
  track_inventory boolean NOT NULL DEFAULT true,
  track_serial    boolean NOT NULL DEFAULT false,
  garantia_meses  integer,
  stock           integer,
  estado          text NOT NULL DEFAULT 'activo',
  parent_item_id  uuid,
  eliminado_en    timestamptz,
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE inventory.configuracion_inventario (
  empresa_id                uuid PRIMARY KEY,
  metodo_costeo             text NOT NULL DEFAULT 'promedio',
  permitir_stock_negativo   boolean NOT NULL DEFAULT false,
  alerta_stock_bajo         boolean NOT NULL DEFAULT true,
  actualizar_costo_en_compra boolean NOT NULL DEFAULT true,
  unidad_defecto            text NOT NULL DEFAULT 'Unidad (u)',
  actualizado_en            timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE inventory.proveedores (
  id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id uuid NOT NULL,
  codigo     text NOT NULL,
  nombre     text NOT NULL,
  correo     text NOT NULL DEFAULT '',
  telefono   text NOT NULL DEFAULT '',
  activo     boolean NOT NULL DEFAULT true
);

CREATE TABLE inventory.lotes_inventario (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id          uuid NOT NULL,
  item_id             uuid NOT NULL,
  proveedor_id        uuid,
  codigo_lote         text NOT NULL,
  vence_en            date,
  cantidad_inicial    integer NOT NULL DEFAULT 0,
  cantidad_disponible integer NOT NULL DEFAULT 0,
  creado_en           timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en      timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE inventory.reservas_ordenes (
  order_id   uuid PRIMARY KEY,
  items_json jsonb NOT NULL,
  creado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE inventory.ubicaciones (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  codigo         text NOT NULL,
  nombre         text NOT NULL,
  tipo           text NOT NULL DEFAULT 'bodega',
  direccion      text NOT NULL DEFAULT '',
  predeterminada boolean NOT NULL DEFAULT false,
  activo         boolean NOT NULL DEFAULT true,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, codigo)
);

CREATE TABLE inventory.item_stock_ubicacion (
  empresa_id     uuid NOT NULL,
  item_id        uuid NOT NULL,
  ubicacion_id   uuid NOT NULL,
  stock          integer NOT NULL DEFAULT 0,
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (item_id, ubicacion_id)
);

CREATE TABLE inventory.movimientos_inventario (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id        uuid NOT NULL,
  tipo              text NOT NULL,
  motivo            text NOT NULL DEFAULT '',
  referencia        text NOT NULL DEFAULT '',
  ubicacion_origen  text NOT NULL DEFAULT '',
  ubicacion_destino text NOT NULL DEFAULT '',
  lineas            jsonb NOT NULL DEFAULT '[]'::jsonb,
  creado_por        uuid,
  creado_en         timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE inventory.ajustes_stock (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id        uuid NOT NULL,
  item_id           uuid NOT NULL,
  cantidad_anterior integer NOT NULL,
  cantidad_nueva    integer NOT NULL,
  motivo            text NOT NULL DEFAULT '',
  realizado_en      timestamptz NOT NULL DEFAULT NOW()
);


CREATE TABLE item.item_componentes (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL,
  item_id            uuid NOT NULL,
  componente_item_id uuid NOT NULL,
  cantidad           integer NOT NULL DEFAULT 1
);

CREATE TABLE item.item_series (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  item_id        uuid NOT NULL,
  serial         text NOT NULL,
  estado         text NOT NULL DEFAULT 'disponible',
  order_id       uuid,
  ubicacion_id   uuid,
  vendido_en     timestamptz,
  devuelto_en    timestamptz,
  garantia_hasta date,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE inventory.reservas_lotes (
  order_id   uuid NOT NULL,
  lote_id    uuid NOT NULL,
  empresa_id uuid NOT NULL,
  item_id    uuid NOT NULL,
  cantidad   integer NOT NULL
);

CREATE TABLE inventory.eventos_procesados (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE inventory.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente',
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);

-- Triggers de propiedad del stock (migración 84).
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
