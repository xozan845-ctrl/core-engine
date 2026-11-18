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
  negocio_id      uuid,
  tipo            text NOT NULL DEFAULT 'producto',
  codigo          text NOT NULL,
  nombre          text NOT NULL,
  track_inventory boolean NOT NULL DEFAULT true,
  track_serial    boolean NOT NULL DEFAULT false,
  garantia_meses  integer,
  estado          text NOT NULL DEFAULT 'activo',
  parent_item_id  uuid,
  eliminado_en    timestamptz,
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE inventory.configuracion_inventario (
  empresa_id                uuid PRIMARY KEY,
  negocio_id                uuid,
  metodo_costeo             text NOT NULL DEFAULT 'promedio',
  permitir_stock_negativo   boolean NOT NULL DEFAULT false,
  alerta_stock_bajo         boolean NOT NULL DEFAULT true,
  actualizar_costo_en_compra boolean NOT NULL DEFAULT true,
  unidad_defecto            text NOT NULL DEFAULT 'Unidad (u)',
  actualizado_en            timestamptz NOT NULL DEFAULT NOW()
);
ALTER TABLE inventory.configuracion_inventario ADD COLUMN IF NOT EXISTS negocio_id uuid;
CREATE UNIQUE INDEX IF NOT EXISTS idx_configuracion_inventario_empresa_negocio
  ON inventory.configuracion_inventario (empresa_id,
    COALESCE(negocio_id, '00000000-0000-0000-0000-000000000000'::uuid));

CREATE TABLE inventory.proveedores (
  id         uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id uuid NOT NULL,
  negocio_id uuid,
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
  negocio_id     uuid,
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

CREATE TABLE IF NOT EXISTS inventory.devoluciones_procesadas (
  devolucion_id uuid PRIMARY KEY,
  order_id uuid NOT NULL,
  procesado_en timestamptz NOT NULL DEFAULT NOW()
);
