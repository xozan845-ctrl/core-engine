DROP SCHEMA IF EXISTS item CASCADE;
DROP SCHEMA IF EXISTS pricing CASCADE;
CREATE SCHEMA item;
CREATE SCHEMA pricing;
CREATE EXTENSION IF NOT EXISTS btree_gist;

CREATE TABLE item.items (
  id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id    uuid NOT NULL,
  tipo          text NOT NULL DEFAULT 'producto',
  codigo        text NOT NULL,
  nombre        text NOT NULL,
  costo_cents   integer NOT NULL DEFAULT 0,
  tipo_precio   text NOT NULL DEFAULT 'fijo',
  tipo_costo    text NOT NULL DEFAULT 'fijo',
  estado        text NOT NULL DEFAULT 'activo',
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en  timestamptz
);

CREATE TABLE pricing.listas_precio (
  id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id       uuid NOT NULL,
  codigo           text NOT NULL,
  nombre           text NOT NULL,
  descripcion      text NOT NULL DEFAULT '',
  moneda           text NOT NULL DEFAULT 'NIO',
  segmento_codigo  text NOT NULL DEFAULT '',
  predeterminada   boolean NOT NULL DEFAULT false,
  activo           boolean NOT NULL DEFAULT true,
  creado_en        timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en   timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX ux_listas_precio_empresa_codigo ON pricing.listas_precio (empresa_id, lower(codigo));
CREATE UNIQUE INDEX ux_listas_precio_predeterminada ON pricing.listas_precio (empresa_id) WHERE predeterminada AND activo;
CREATE UNIQUE INDEX ux_listas_precio_segmento ON pricing.listas_precio (empresa_id, segmento_codigo) WHERE segmento_codigo <> '' AND activo;

CREATE TABLE pricing.lista_precio_items (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  lista_id        uuid REFERENCES pricing.listas_precio(id) ON DELETE CASCADE,
  item_id         uuid NOT NULL,
  desde_cantidad  integer NOT NULL DEFAULT 1 CHECK (desde_cantidad >= 1),
  hasta_cantidad  integer CHECK (hasta_cantidad IS NULL OR hasta_cantidad >= desde_cantidad),
  precio_cents    integer NOT NULL CHECK (precio_cents >= 0),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX ux_lista_precio_items_lista ON pricing.lista_precio_items (lista_id, item_id, desde_cantidad) WHERE lista_id IS NOT NULL;
CREATE UNIQUE INDEX ux_lista_precio_items_base ON pricing.lista_precio_items (empresa_id, item_id, desde_cantidad) WHERE lista_id IS NULL;

CREATE TABLE pricing.item_tarifas (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  item_id        uuid NOT NULL,
  desde          date NOT NULL,
  hasta          date NOT NULL,
  precio_cents   integer NOT NULL CHECK (precio_cents >= 0),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  CONSTRAINT item_tarifas_rango_check CHECK (hasta >= desde)
);

CREATE TABLE pricing.promociones (
  id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id           uuid NOT NULL,
  nombre               text NOT NULL,
  tipo                 text NOT NULL,
  item_id              uuid,
  sku                  text NOT NULL DEFAULT '',
  cantidad_requerida   integer,
  cantidad_gratis      integer,
  descuento_porcentaje integer,
  fecha_desde          date NOT NULL,
  fecha_hasta          date NOT NULL,
  estado               text NOT NULL DEFAULT 'activo',
  creado_en            timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en       timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en         timestamptz,
  CHECK (fecha_hasta >= fecha_desde)
);

CREATE TABLE pricing.outbox (
  event_id     uuid PRIMARY KEY,
  tipo         text NOT NULL,
  payload      jsonb NOT NULL,
  estado       text NOT NULL DEFAULT 'pendiente',
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  publicado_en timestamptz
);

CREATE TABLE IF NOT EXISTS pricing.historico_precios_base (
  id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id            uuid NOT NULL,
  item_id               uuid NOT NULL,
  precio_anterior_cents integer,
  precio_nuevo_cents    integer NOT NULL,
  vigente_desde         timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS pricing.cupones (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id uuid NOT NULL,
  codigo text NOT NULL,
  nombre text NOT NULL,
  tipo text NOT NULL,
  valor integer NOT NULL,
  minimo_compra_cents integer NOT NULL DEFAULT 0,
  maximo_descuento_cents integer,
  usos_maximos integer,
  usos_actuales integer NOT NULL DEFAULT 0,
  fecha_desde date NOT NULL,
  fecha_hasta date NOT NULL,
  activo boolean NOT NULL DEFAULT true,
  creado_en timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en timestamptz
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_cupones_empresa_codigo_test
  ON pricing.cupones (empresa_id, upper(codigo)) WHERE eliminado_en IS NULL;

CREATE TABLE IF NOT EXISTS pricing.cupon_usos (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  cupon_id uuid NOT NULL,
  empresa_id uuid NOT NULL,
  order_id uuid NOT NULL,
  codigo text NOT NULL,
  monto_descuento_cents integer NOT NULL DEFAULT 0,
  estado text NOT NULL DEFAULT 'usado',
  creado_en timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_cupon_usos_cupon_orden_test
  ON pricing.cupon_usos (cupon_id, order_id);

-- ADR-0051: API de dominio que pricing usa en lugar de escribir item.items.
CREATE OR REPLACE FUNCTION item.marcar_tipo_precio(
  p_empresa_id uuid, p_item_id uuid, p_tipo text
) RETURNS integer LANGUAGE plpgsql AS $$
DECLARE actualizados integer;
BEGIN
  UPDATE item.items SET tipo_precio=p_tipo, actualizado_en=NOW()
   WHERE id=p_item_id AND empresa_id=p_empresa_id AND tipo_precio<>p_tipo;
  GET DIAGNOSTICS actualizados = ROW_COUNT;
  RETURN actualizados;
END $$;
