-- RMA / devolución parcial por línea.
--
-- La devolución total existente cancela la orden y compensa la comisión. Esta
-- fase añade una entidad RMA con líneas y cantidades, restock idempotente en
-- inventory y compensación proporcional de comisión.
BEGIN;

CREATE TABLE IF NOT EXISTS orders.devoluciones (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  order_id       uuid NOT NULL,
  empresa_id     uuid NOT NULL,
  negocio_id     uuid,
  motivo         text NOT NULL DEFAULT '',
  estado         text NOT NULL DEFAULT 'solicitada'
                 CHECK (estado IN ('solicitada', 'aprobada', 'recibida', 'rechazada')),
  total_cents    integer NOT NULL DEFAULT 0 CHECK (total_cents >= 0),
  moneda         text NOT NULL DEFAULT 'NIO',
  creado_por     uuid NOT NULL,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_devoluciones_order
  ON orders.devoluciones (order_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_devoluciones_empresa
  ON orders.devoluciones (empresa_id, estado, creado_en DESC);

CREATE TABLE IF NOT EXISTS orders.devolucion_lineas (
  id                    uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  devolucion_id         uuid NOT NULL REFERENCES orders.devoluciones(id) ON DELETE CASCADE,
  oferta_id             uuid NOT NULL,
  sku                   text NOT NULL,
  cantidad              integer NOT NULL CHECK (cantidad > 0),
  precio_unitario_cents integer NOT NULL CHECK (precio_unitario_cents >= 0),
  descuento_cents       integer NOT NULL DEFAULT 0 CHECK (descuento_cents >= 0),
  total_cents           integer NOT NULL CHECK (total_cents >= 0),
  restock               boolean NOT NULL DEFAULT true,
  ubicacion_id          uuid
);
CREATE INDEX IF NOT EXISTS idx_devolucion_lineas_devolucion
  ON orders.devolucion_lineas (devolucion_id);

CREATE TABLE IF NOT EXISTS inventory.devoluciones_procesadas (
  devolucion_id uuid PRIMARY KEY,
  order_id      uuid NOT NULL,
  procesado_en  timestamptz NOT NULL DEFAULT NOW()
);

ALTER TABLE commissions.comisiones
  ADD COLUMN IF NOT EXISTS cantidad integer NOT NULL DEFAULT 1;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON orders.devoluciones TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON orders.devolucion_lineas TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON inventory.devoluciones_procesadas TO core_engine_app;
  END IF;
END $$;

COMMIT;
