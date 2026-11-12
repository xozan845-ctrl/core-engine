-- Operaciones avanzadas de catálogo (R-PS-04): configuración de inventario,
-- importación, escaneo, combos, promociones y movimientos/traslados.
-- Migración aditiva e idempotente (R-DB-6).
BEGIN;

-- ── configuración de inventario por empresa ──────────────────────────────
CREATE TABLE IF NOT EXISTS catalog.configuracion_inventario (
  empresa_id                uuid PRIMARY KEY,
  metodo_costeo             text NOT NULL DEFAULT 'promedio'
                              CHECK (metodo_costeo IN ('promedio', 'fifo', 'ueps')),
  permitir_stock_negativo   boolean NOT NULL DEFAULT false,
  alerta_stock_bajo         boolean NOT NULL DEFAULT true,
  actualizar_costo_en_compra boolean NOT NULL DEFAULT true,
  unidad_defecto            text NOT NULL DEFAULT 'Unidad (u)',
  actualizado_en            timestamptz NOT NULL DEFAULT NOW()
);

-- ── combos: agrupación vendible de productos ─────────────────────────────
CREATE TABLE IF NOT EXISTS catalog.combos (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  sku            text NOT NULL,
  nombre         text NOT NULL,
  descripcion    text NOT NULL DEFAULT '',
  codigo_barras  text NOT NULL DEFAULT '',
  precio_cents   integer NOT NULL CHECK (precio_cents >= 0),
  estado         text NOT NULL DEFAULT 'activo' CHECK (estado IN ('activo', 'inactivo')),
  componentes    jsonb NOT NULL DEFAULT '[]'::jsonb,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en   timestamptz
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_combos_empresa_sku
  ON catalog.combos (empresa_id, upper(sku)) WHERE eliminado_en IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_combos_empresa_barcode
  ON catalog.combos (empresa_id, codigo_barras)
  WHERE codigo_barras <> '' AND eliminado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_combos_empresa_estado
  ON catalog.combos (empresa_id, estado, creado_en DESC) WHERE eliminado_en IS NULL;

-- ── promociones ──────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS catalog.promociones (
  id                   uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id           uuid NOT NULL,
  nombre               text NOT NULL,
  tipo                 text NOT NULL
                         CHECK (tipo IN ('2x1', 'descuento_cantidad', 'descuento_porcentaje', 'regalo')),
  producto_id          uuid REFERENCES catalog.productos(id) ON DELETE SET NULL,
  sku                  text NOT NULL DEFAULT '',
  cantidad_requerida   integer CHECK (cantidad_requerida IS NULL OR cantidad_requerida >= 1),
  cantidad_gratis      integer CHECK (cantidad_gratis IS NULL OR cantidad_gratis >= 1),
  descuento_porcentaje integer CHECK (descuento_porcentaje IS NULL OR descuento_porcentaje BETWEEN 0 AND 100),
  fecha_desde          date NOT NULL,
  fecha_hasta          date NOT NULL,
  estado               text NOT NULL DEFAULT 'activo' CHECK (estado IN ('activo', 'inactivo')),
  creado_en            timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en       timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en         timestamptz,
  CHECK (fecha_hasta >= fecha_desde)
);
CREATE INDEX IF NOT EXISTS idx_promociones_empresa_estado
  ON catalog.promociones (empresa_id, estado, fecha_desde, fecha_hasta)
  WHERE eliminado_en IS NULL;

-- ── movimientos y traslados de inventario ────────────────────────────────
CREATE TABLE IF NOT EXISTS catalog.movimientos_inventario (
  id                uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id        uuid NOT NULL,
  tipo              text NOT NULL CHECK (tipo IN ('entrada', 'salida', 'ajuste', 'traslado')),
  motivo            text NOT NULL DEFAULT '',
  referencia        text NOT NULL DEFAULT '',
  ubicacion_origen  text NOT NULL DEFAULT '',
  ubicacion_destino text NOT NULL DEFAULT '',
  lineas            jsonb NOT NULL DEFAULT '[]'::jsonb,
  creado_por        text,
  creado_en         timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_movimientos_empresa_fecha
  ON catalog.movimientos_inventario (empresa_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_movimientos_empresa_tipo
  ON catalog.movimientos_inventario (empresa_id, tipo, creado_en DESC);

COMMIT;
