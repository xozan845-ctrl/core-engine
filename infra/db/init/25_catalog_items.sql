-- Plataforma multi-vertical · catálogo unificado (fase 2).
-- Mantiene productos y servicios como fuentes legacy y agrega una superficie
-- estable para producto, servicio y alquiler. Migración aditiva e idempotente.
BEGIN;

ALTER TABLE catalog.servicios
  ADD COLUMN IF NOT EXISTS atributos jsonb NOT NULL DEFAULT '{}'::jsonb;

CREATE TABLE IF NOT EXISTS catalog.items (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL,
  tipo               text NOT NULL CHECK (tipo IN ('producto', 'servicio', 'alquiler')),
  codigo             text NOT NULL,
  nombre             text NOT NULL,
  descripcion        text NOT NULL DEFAULT '',
  categoria          text NOT NULL DEFAULT '',
  precio_cents       integer NOT NULL CHECK (precio_cents >= 0),
  costo_cents        integer NOT NULL DEFAULT 0 CHECK (costo_cents >= 0),
  track_inventory    boolean NOT NULL DEFAULT false,
  stock              integer CHECK (stock IS NULL OR stock >= 0),
  duration_minutes   integer CHECK (duration_minutes IS NULL OR duration_minutes BETWEEN 1 AND 1440),
  atributos          jsonb NOT NULL DEFAULT '{}'::jsonb,
  estado             text NOT NULL DEFAULT 'activo' CHECK (estado IN ('activo', 'inactivo')),
  source_type        text CHECK (source_type IS NULL OR source_type IN ('producto', 'servicio')),
  source_id          uuid,
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en       timestamptz,
  CHECK ((tipo = 'servicio' AND duration_minutes IS NOT NULL)
      OR (tipo <> 'servicio' AND duration_minutes IS NULL)),
  CHECK ((source_type IS NULL AND source_id IS NULL)
      OR (source_type IS NOT NULL AND source_id IS NOT NULL))
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_items_empresa_tipo_codigo
  ON catalog.items (empresa_id, tipo, upper(codigo))
  WHERE eliminado_en IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_items_source
  ON catalog.items (source_type, source_id)
  WHERE source_type IS NOT NULL AND source_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_items_empresa_tipo_estado
  ON catalog.items (empresa_id, tipo, estado, creado_en DESC)
  WHERE eliminado_en IS NULL;

CREATE OR REPLACE VIEW catalog.items_vista AS
SELECT i.id AS item_id,
       i.empresa_id,
       i.tipo,
       i.codigo,
       i.nombre,
       i.descripcion,
       i.categoria,
       i.precio_cents,
       i.costo_cents,
       i.track_inventory,
       i.stock,
       i.duration_minutes,
       i.atributos,
       i.estado,
       i.source_type,
       i.source_id,
       i.creado_en,
       i.actualizado_en
  FROM catalog.items i
 WHERE i.eliminado_en IS NULL;

CREATE TABLE IF NOT EXISTS catalog.item_atributo_esquemas (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid,
  tipo               text NOT NULL CHECK (tipo IN ('producto', 'servicio', 'alquiler')),
  categoria          text NOT NULL,
  esquema            jsonb NOT NULL,
  version            integer NOT NULL DEFAULT 1 CHECK (version >= 1),
  activo             boolean NOT NULL DEFAULT true,
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_item_atributo_esquemas_scope
  ON catalog.item_atributo_esquemas
  (COALESCE(empresa_id, '00000000-0000-0000-0000-000000000000'::uuid), tipo, lower(categoria), version);
CREATE INDEX IF NOT EXISTS idx_item_atributo_esquemas_lookup
  ON catalog.item_atributo_esquemas (empresa_id, tipo, lower(categoria), activo);

INSERT INTO catalog.item_atributo_esquemas (empresa_id, tipo, categoria, esquema)
VALUES
  (NULL, 'producto', 'general', '{"type":"object","additionalProperties":true}'::jsonb),
  (NULL, 'servicio', 'general', '{"type":"object","additionalProperties":true}'::jsonb),
  (NULL, 'alquiler', 'general', '{"type":"object","additionalProperties":true}'::jsonb)
ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS catalog.proveedores (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL,
  codigo             text NOT NULL,
  nombre             text NOT NULL,
  correo             text NOT NULL DEFAULT '',
  telefono           text NOT NULL DEFAULT '',
  activo             boolean NOT NULL DEFAULT true,
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_proveedores_empresa_codigo
  ON catalog.proveedores (empresa_id, upper(codigo));

CREATE TABLE IF NOT EXISTS catalog.item_proveedores (
  item_id            uuid NOT NULL REFERENCES catalog.items(id) ON DELETE CASCADE,
  proveedor_id       uuid NOT NULL REFERENCES catalog.proveedores(id) ON DELETE CASCADE,
  codigo_proveedor   text NOT NULL DEFAULT '',
  costo_cents        integer CHECK (costo_cents IS NULL OR costo_cents >= 0),
  es_preferido       boolean NOT NULL DEFAULT false,
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (item_id, proveedor_id)
);

CREATE TABLE IF NOT EXISTS catalog.lotes_inventario (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL,
  item_id            uuid NOT NULL REFERENCES catalog.items(id),
  proveedor_id       uuid REFERENCES catalog.proveedores(id),
  codigo_lote        text NOT NULL,
  vence_en           date,
  cantidad_inicial   integer NOT NULL CHECK (cantidad_inicial >= 0),
  cantidad_disponible integer NOT NULL CHECK (cantidad_disponible >= 0),
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, item_id, codigo_lote),
  CHECK (cantidad_disponible <= cantidad_inicial)
);
CREATE INDEX IF NOT EXISTS idx_lotes_empresa_vencimiento
  ON catalog.lotes_inventario (empresa_id, vence_en, codigo_lote);

-- Backfill idempotente de las fuentes existentes.
INSERT INTO catalog.items
  (empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
   costo_cents, track_inventory, stock, duration_minutes, atributos, estado,
   source_type, source_id, creado_en, actualizado_en)
SELECT p.empresa_id, 'producto', p.sku, p.nombre, p.descripcion, p.categoria,
       p.precio_base_cents, p.costo_cents, true, p.stock, NULL, p.atributos,
       p.estado_operativo, 'producto', p.id, p.creado_en, p.actualizado_en
  FROM catalog.productos p
ON CONFLICT (source_type, source_id)
  WHERE source_type IS NOT NULL AND source_id IS NOT NULL DO UPDATE SET
  empresa_id = EXCLUDED.empresa_id,
  codigo = EXCLUDED.codigo,
  nombre = EXCLUDED.nombre,
  descripcion = EXCLUDED.descripcion,
  categoria = EXCLUDED.categoria,
  precio_cents = EXCLUDED.precio_cents,
  costo_cents = EXCLUDED.costo_cents,
  track_inventory = EXCLUDED.track_inventory,
  stock = EXCLUDED.stock,
  atributos = EXCLUDED.atributos,
  estado = EXCLUDED.estado,
  actualizado_en = NOW(),
  eliminado_en = NULL;

INSERT INTO catalog.items
  (empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
   costo_cents, track_inventory, stock, duration_minutes, atributos, estado,
   source_type, source_id, creado_en, actualizado_en)
SELECT s.empresa_id, 'servicio', s.codigo, s.nombre, s.descripcion, s.categoria,
       s.precio_cents, s.costo_cents, false, NULL, s.duracion_minutos,
       s.atributos, s.estado, 'servicio', s.id, s.creado_en, s.actualizado_en
  FROM catalog.servicios s
ON CONFLICT (source_type, source_id)
  WHERE source_type IS NOT NULL AND source_id IS NOT NULL DO UPDATE SET
  empresa_id = EXCLUDED.empresa_id,
  codigo = EXCLUDED.codigo,
  nombre = EXCLUDED.nombre,
  descripcion = EXCLUDED.descripcion,
  categoria = EXCLUDED.categoria,
  precio_cents = EXCLUDED.precio_cents,
  costo_cents = EXCLUDED.costo_cents,
  track_inventory = EXCLUDED.track_inventory,
  stock = EXCLUDED.stock,
  duration_minutes = EXCLUDED.duration_minutes,
  atributos = EXCLUDED.atributos,
  estado = EXCLUDED.estado,
  actualizado_en = NOW(),
  eliminado_en = NULL;

CREATE OR REPLACE FUNCTION catalog.fn_sync_item_producto() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    UPDATE catalog.items
       SET estado = 'inactivo', eliminado_en = NOW(), actualizado_en = NOW()
     WHERE source_type = 'producto' AND source_id = OLD.id;
    RETURN OLD;
  END IF;

  INSERT INTO catalog.items
    (empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
     costo_cents, track_inventory, stock, atributos, estado, source_type,
     source_id, creado_en, actualizado_en, eliminado_en)
  VALUES
    (NEW.empresa_id, 'producto', NEW.sku, NEW.nombre, NEW.descripcion,
     NEW.categoria, NEW.precio_base_cents, NEW.costo_cents, true, NEW.stock,
     NEW.atributos, NEW.estado_operativo, 'producto', NEW.id, NEW.creado_en,
     NOW(), NULL)
  ON CONFLICT (source_type, source_id)
    WHERE source_type IS NOT NULL AND source_id IS NOT NULL DO UPDATE SET
    empresa_id = EXCLUDED.empresa_id, codigo = EXCLUDED.codigo,
    nombre = EXCLUDED.nombre, descripcion = EXCLUDED.descripcion,
    categoria = EXCLUDED.categoria, precio_cents = EXCLUDED.precio_cents,
    costo_cents = EXCLUDED.costo_cents, track_inventory = true,
    stock = EXCLUDED.stock, atributos = EXCLUDED.atributos,
    estado = EXCLUDED.estado, actualizado_en = NOW(), eliminado_en = NULL;
  RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION catalog.fn_sync_item_servicio() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'DELETE' THEN
    UPDATE catalog.items
       SET estado = 'inactivo', eliminado_en = NOW(), actualizado_en = NOW()
     WHERE source_type = 'servicio' AND source_id = OLD.id;
    RETURN OLD;
  END IF;

  INSERT INTO catalog.items
    (empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
     costo_cents, track_inventory, duration_minutes, atributos, estado,
     source_type, source_id, creado_en, actualizado_en, eliminado_en)
  VALUES
    (NEW.empresa_id, 'servicio', NEW.codigo, NEW.nombre, NEW.descripcion,
     NEW.categoria, NEW.precio_cents, NEW.costo_cents, false,
     NEW.duracion_minutos, NEW.atributos, NEW.estado, 'servicio', NEW.id,
     NEW.creado_en, NOW(), NEW.eliminado_en)
  ON CONFLICT (source_type, source_id)
    WHERE source_type IS NOT NULL AND source_id IS NOT NULL DO UPDATE SET
    empresa_id = EXCLUDED.empresa_id, codigo = EXCLUDED.codigo,
    nombre = EXCLUDED.nombre, descripcion = EXCLUDED.descripcion,
    categoria = EXCLUDED.categoria, precio_cents = EXCLUDED.precio_cents,
    costo_cents = EXCLUDED.costo_cents, track_inventory = false,
    duration_minutes = EXCLUDED.duration_minutes,
    atributos = EXCLUDED.atributos, estado = EXCLUDED.estado,
    actualizado_en = NOW(), eliminado_en = EXCLUDED.eliminado_en;
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS trg_sync_item_producto ON catalog.productos;
CREATE TRIGGER trg_sync_item_producto
AFTER INSERT OR UPDATE OR DELETE ON catalog.productos
FOR EACH ROW EXECUTE FUNCTION catalog.fn_sync_item_producto();

DROP TRIGGER IF EXISTS trg_sync_item_servicio ON catalog.servicios;
CREATE TRIGGER trg_sync_item_servicio
AFTER INSERT OR UPDATE OR DELETE ON catalog.servicios
FOR EACH ROW EXECUTE FUNCTION catalog.fn_sync_item_servicio();

COMMIT;
