-- ADR-0018 · corte de inventario a su propio esquema.
--
-- Copia las tablas de inventario de `catalog` a `inventory` sin romper el
-- rollback: las tablas viejas se conservan hasta el PR de limpieza. El servicio
-- `inventory-service` usa `inventory.*`; `catalog.items` sigue siendo el dato
-- maestro del ítem y conserva `stock` como proyección transitoria hasta la
-- extracción de `item-service` (PR 8 de ADR-0018).
--
-- Idempotente para local/staging: crea si falta y copia con ON CONFLICT.

CREATE SCHEMA IF NOT EXISTS inventory;

-- ── configuración y proveedores ───────────────────────────────────────────
CREATE TABLE IF NOT EXISTS inventory.configuracion_inventario
  (LIKE catalog.configuracion_inventario INCLUDING ALL);
INSERT INTO inventory.configuracion_inventario
SELECT * FROM catalog.configuracion_inventario
ON CONFLICT (empresa_id) DO NOTHING;

CREATE TABLE IF NOT EXISTS inventory.proveedores
  (LIKE catalog.proveedores INCLUDING ALL);
INSERT INTO inventory.proveedores
SELECT * FROM catalog.proveedores
ON CONFLICT (id) DO NOTHING;

-- ── ubicaciones y stock por ubicación ──────────────────────────────────────
CREATE TABLE IF NOT EXISTS inventory.ubicaciones
  (LIKE catalog.ubicaciones INCLUDING ALL);
INSERT INTO inventory.ubicaciones
SELECT * FROM catalog.ubicaciones
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS inventory.item_stock_ubicacion
  (LIKE catalog.item_stock_ubicacion INCLUDING ALL);
INSERT INTO inventory.item_stock_ubicacion
SELECT * FROM catalog.item_stock_ubicacion
ON CONFLICT (item_id, ubicacion_id) DO NOTHING;

-- ── lotes, movimientos y ajustes ───────────────────────────────────────────
CREATE TABLE IF NOT EXISTS inventory.lotes_inventario
  (LIKE catalog.lotes_inventario INCLUDING ALL);
INSERT INTO inventory.lotes_inventario
SELECT * FROM catalog.lotes_inventario
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS inventory.movimientos_inventario
  (LIKE catalog.movimientos_inventario INCLUDING ALL);
INSERT INTO inventory.movimientos_inventario
SELECT * FROM catalog.movimientos_inventario
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS inventory.ajustes_stock
  (LIKE catalog.ajustes_stock INCLUDING ALL);
INSERT INTO inventory.ajustes_stock
SELECT * FROM catalog.ajustes_stock
ON CONFLICT (id) DO NOTHING;

-- ── reservas de la saga e idempotencia ─────────────────────────────────────
CREATE TABLE IF NOT EXISTS inventory.reservas_ordenes
  (LIKE catalog.reservas_ordenes INCLUDING ALL);
INSERT INTO inventory.reservas_ordenes
SELECT * FROM catalog.reservas_ordenes
ON CONFLICT (order_id) DO NOTHING;

CREATE TABLE IF NOT EXISTS inventory.reservas_lotes
  (LIKE catalog.reservas_lotes INCLUDING ALL);
INSERT INTO inventory.reservas_lotes AS r
SELECT * FROM catalog.reservas_lotes c
WHERE NOT EXISTS (
  SELECT 1 FROM inventory.reservas_lotes r2
   WHERE r2.order_id = c.order_id AND r2.lote_id = c.lote_id
);

CREATE TABLE IF NOT EXISTS inventory.eventos_procesados
  (LIKE catalog.eventos_procesados INCLUDING ALL);
INSERT INTO inventory.eventos_procesados
SELECT * FROM catalog.eventos_procesados
ON CONFLICT (event_id) DO NOTHING;

CREATE TABLE IF NOT EXISTS inventory.outbox
  (LIKE catalog.outbox INCLUDING ALL);
INSERT INTO inventory.outbox
SELECT * FROM catalog.outbox
ON CONFLICT (event_id) DO NOTHING;

-- ── claves foráneas que `LIKE` no copia ───────────────────────────────────
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
     WHERE conname = 'inventory_lotes_proveedor_fk'
       AND conrelid = 'inventory.lotes_inventario'::regclass
  ) THEN
    ALTER TABLE inventory.lotes_inventario
      ADD CONSTRAINT inventory_lotes_proveedor_fk
      FOREIGN KEY (proveedor_id) REFERENCES inventory.proveedores(id)
      ON DELETE SET NULL NOT VALID;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
     WHERE conname = 'inventory_stock_ubicacion_fk'
       AND conrelid = 'inventory.item_stock_ubicacion'::regclass
  ) THEN
    ALTER TABLE inventory.item_stock_ubicacion
      ADD CONSTRAINT inventory_stock_ubicacion_fk
      FOREIGN KEY (ubicacion_id) REFERENCES inventory.ubicaciones(id)
      ON DELETE CASCADE NOT VALID;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
     WHERE conname = 'inventory_reservas_lote_fk'
       AND conrelid = 'inventory.reservas_lotes'::regclass
  ) THEN
    ALTER TABLE inventory.reservas_lotes
      ADD CONSTRAINT inventory_reservas_lote_fk
      FOREIGN KEY (lote_id) REFERENCES inventory.lotes_inventario(id)
      ON DELETE CASCADE NOT VALID;
  END IF;
END $$;

-- `item_series` sigue en catalog (lo reclamará item-service), pero su
-- ubicación ya es de inventario.
DO $$
BEGIN
  IF to_regclass('catalog.item_series') IS NOT NULL THEN
    ALTER TABLE catalog.item_series
      DROP CONSTRAINT IF EXISTS item_series_ubicacion_id_fkey;
    IF NOT EXISTS (
      SELECT 1 FROM pg_constraint
       WHERE conname = 'item_series_ubicacion_id_fkey'
         AND conrelid = 'catalog.item_series'::regclass
    ) THEN
      ALTER TABLE catalog.item_series
        ADD CONSTRAINT item_series_ubicacion_id_fkey
        FOREIGN KEY (ubicacion_id) REFERENCES inventory.ubicaciones(id)
        ON DELETE SET NULL NOT VALID;
    END IF;
  END IF;
END $$;

-- ── grants y RLS ──────────────────────────────────────────────────────────
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA inventory TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA inventory TO core_engine_app;
  END IF;
END $$;

ALTER TABLE inventory.configuracion_inventario ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS configuracion_inventario_admin ON inventory.configuracion_inventario;
DROP POLICY IF EXISTS configuracion_inventario_empresa ON inventory.configuracion_inventario;
CREATE POLICY configuracion_inventario_admin ON inventory.configuracion_inventario FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY configuracion_inventario_empresa ON inventory.configuracion_inventario FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.proveedores ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS proveedores_admin ON inventory.proveedores;
DROP POLICY IF EXISTS proveedores_empresa ON inventory.proveedores;
CREATE POLICY proveedores_admin ON inventory.proveedores FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY proveedores_empresa ON inventory.proveedores FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.lotes_inventario ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS lotes_inventario_admin ON inventory.lotes_inventario;
DROP POLICY IF EXISTS lotes_inventario_empresa ON inventory.lotes_inventario;
CREATE POLICY lotes_inventario_admin ON inventory.lotes_inventario FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY lotes_inventario_empresa ON inventory.lotes_inventario FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.ubicaciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS ubicaciones_admin ON inventory.ubicaciones;
DROP POLICY IF EXISTS ubicaciones_empresa ON inventory.ubicaciones;
CREATE POLICY ubicaciones_admin ON inventory.ubicaciones FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY ubicaciones_empresa ON inventory.ubicaciones FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.item_stock_ubicacion ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_stock_ubicacion_admin ON inventory.item_stock_ubicacion;
DROP POLICY IF EXISTS item_stock_ubicacion_empresa ON inventory.item_stock_ubicacion;
CREATE POLICY item_stock_ubicacion_admin ON inventory.item_stock_ubicacion FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY item_stock_ubicacion_empresa ON inventory.item_stock_ubicacion FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.movimientos_inventario ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS movimientos_inventario_admin ON inventory.movimientos_inventario;
DROP POLICY IF EXISTS movimientos_inventario_empresa ON inventory.movimientos_inventario;
CREATE POLICY movimientos_inventario_admin ON inventory.movimientos_inventario FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY movimientos_inventario_empresa ON inventory.movimientos_inventario FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.ajustes_stock ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS ajustes_stock_admin ON inventory.ajustes_stock;
DROP POLICY IF EXISTS ajustes_stock_empresa ON inventory.ajustes_stock;
CREATE POLICY ajustes_stock_admin ON inventory.ajustes_stock FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY ajustes_stock_empresa ON inventory.ajustes_stock FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.reservas_lotes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS reservas_lotes_admin ON inventory.reservas_lotes;
DROP POLICY IF EXISTS reservas_lotes_empresa ON inventory.reservas_lotes;
CREATE POLICY reservas_lotes_admin ON inventory.reservas_lotes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY reservas_lotes_empresa ON inventory.reservas_lotes FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.reservas_ordenes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS reservas_ordenes_admin ON inventory.reservas_ordenes;
CREATE POLICY reservas_ordenes_admin ON inventory.reservas_ordenes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE inventory.eventos_procesados ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS eventos_procesados_admin ON inventory.eventos_procesados;
CREATE POLICY eventos_procesados_admin ON inventory.eventos_procesados FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE inventory.outbox ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS outbox_admin ON inventory.outbox;
CREATE POLICY outbox_admin ON inventory.outbox FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
