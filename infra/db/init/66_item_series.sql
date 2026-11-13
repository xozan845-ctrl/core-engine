-- Fase P1.3 · serial/IMEI por unidad y trazabilidad de venta/garantia.
-- Un item serializado mantiene una fila por unidad fisica. La venta la marca
-- como vendida y calcula la garantia; la devolucion la deja como devuelta.
BEGIN;

ALTER TABLE catalog.items
  ADD COLUMN IF NOT EXISTS track_serial boolean NOT NULL DEFAULT false;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'items_track_serial_check'
  ) THEN
    ALTER TABLE catalog.items
      ADD CONSTRAINT items_track_serial_check
      CHECK (NOT track_serial OR (tipo = 'producto' AND track_inventory));
  END IF;
END $$;

CREATE TABLE IF NOT EXISTS catalog.item_series (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id      uuid NOT NULL,
  item_id         uuid NOT NULL REFERENCES catalog.items(id) ON DELETE CASCADE,
  ubicacion_id    uuid REFERENCES catalog.ubicaciones(id) ON DELETE SET NULL,
  serial          text NOT NULL CHECK (char_length(trim(serial)) BETWEEN 1 AND 120),
  imei            text NOT NULL DEFAULT '',
  estado          text NOT NULL DEFAULT 'disponible'
                  CHECK (estado IN ('disponible', 'reservado', 'vendido', 'devuelto', 'baja')),
  order_id        uuid,
  vendido_en      timestamptz,
  garantia_hasta  date,
  devuelto_en     timestamptz,
  metadata        jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(metadata) = 'object'),
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_item_series_empresa_serial
  ON catalog.item_series (empresa_id, lower(serial));
CREATE UNIQUE INDEX IF NOT EXISTS ux_item_series_empresa_imei
  ON catalog.item_series (empresa_id, imei)
  WHERE imei <> '';
CREATE INDEX IF NOT EXISTS idx_item_series_item_estado
  ON catalog.item_series (empresa_id, item_id, estado);
CREATE INDEX IF NOT EXISTS idx_item_series_order
  ON catalog.item_series (order_id)
  WHERE order_id IS NOT NULL;

COMMENT ON TABLE catalog.item_series IS
  'Una fila por unidad serializada; conserva venta, devolucion y garantia.';

GRANT SELECT, INSERT, UPDATE, DELETE ON catalog.item_series TO core_engine_app;

ALTER TABLE catalog.item_series ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_series_admin ON catalog.item_series;
CREATE POLICY item_series_admin ON catalog.item_series
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS item_series_empresa ON catalog.item_series;
CREATE POLICY item_series_empresa ON catalog.item_series
  FOR ALL TO authenticated
  USING (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  )
  WITH CHECK (
    auth.rol() = 'empresario'
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );
DROP POLICY IF EXISTS item_series_lectura ON catalog.item_series;
CREATE POLICY item_series_lectura ON catalog.item_series
  FOR SELECT TO authenticated
  USING (
    auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
    AND (empresa_id)::text = (auth.jwt() ->> 'tenant_id')
  );

COMMIT;
