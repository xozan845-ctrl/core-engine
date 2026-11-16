-- ADR-0018 · corte del catálogo maestro a `item-service`.
--
-- Copia items, esquemas de atributos, imagenes, componentes, series e
-- historico de precios desde `catalog` a `item`. Precios e inventario ya viven
-- en sus propios esquemas. Las tablas viejas se conservan para rollback hasta
-- la limpieza final.

CREATE SCHEMA IF NOT EXISTS item;

CREATE TABLE IF NOT EXISTS item.items
  (LIKE catalog.items INCLUDING ALL);
INSERT INTO item.items SELECT * FROM catalog.items ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS item.item_atributo_esquemas
  (LIKE catalog.item_atributo_esquemas INCLUDING ALL);
INSERT INTO item.item_atributo_esquemas
SELECT * FROM catalog.item_atributo_esquemas ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS item.item_imagenes
  (LIKE catalog.item_imagenes INCLUDING ALL);
INSERT INTO item.item_imagenes SELECT * FROM catalog.item_imagenes ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS item.item_componentes
  (LIKE catalog.item_componentes INCLUDING ALL);
INSERT INTO item.item_componentes SELECT * FROM catalog.item_componentes ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS item.item_series
  (LIKE catalog.item_series INCLUDING ALL);
INSERT INTO item.item_series SELECT * FROM catalog.item_series ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS item.historico_precios
  (LIKE catalog.historico_precios INCLUDING ALL);
INSERT INTO item.historico_precios
SELECT * FROM catalog.historico_precios ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS item.outbox
  (LIKE catalog.outbox INCLUDING ALL);
INSERT INTO item.outbox SELECT * FROM catalog.outbox ON CONFLICT (event_id) DO NOTHING;

CREATE OR REPLACE VIEW item.items_vista AS
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
       i.creado_en,
       i.actualizado_en
  FROM item.items i
 WHERE i.eliminado_en IS NULL;

-- Claves foráneas que `LIKE` no copia.
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
     WHERE conname = 'item_componentes_item_fk'
       AND conrelid = 'item.item_componentes'::regclass
  ) THEN
    ALTER TABLE item.item_componentes
      ADD CONSTRAINT item_componentes_item_fk
      FOREIGN KEY (item_id) REFERENCES item.items(id) ON DELETE CASCADE NOT VALID;
    ALTER TABLE item.item_componentes
      ADD CONSTRAINT item_componentes_componente_fk
      FOREIGN KEY (componente_item_id) REFERENCES item.items(id) ON DELETE RESTRICT NOT VALID;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
     WHERE conname = 'item_imagenes_item_fk'
       AND conrelid = 'item.item_imagenes'::regclass
  ) THEN
    ALTER TABLE item.item_imagenes
      ADD CONSTRAINT item_imagenes_item_fk
      FOREIGN KEY (item_id) REFERENCES item.items(id) ON DELETE CASCADE NOT VALID;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
     WHERE conname = 'item_series_item_fk'
       AND conrelid = 'item.item_series'::regclass
  ) THEN
    ALTER TABLE item.item_series
      ADD CONSTRAINT item_series_item_fk
      FOREIGN KEY (item_id) REFERENCES item.items(id) ON DELETE CASCADE NOT VALID;
    ALTER TABLE item.item_series
      ADD CONSTRAINT item_series_ubicacion_fk
      FOREIGN KEY (ubicacion_id) REFERENCES inventory.ubicaciones(id) ON DELETE SET NULL NOT VALID;
  END IF;
END $$;

-- Repunta las claves foráneas externas que apuntaban a `catalog.items`: los
-- servicios ya escriben items nuevos en `item.items`, así que una FK a la tabla
-- vieja rechazaría ofertas y referencias legítimas (ADR-0018).
DO $$
BEGIN
  IF to_regclass('stores.ofertas') IS NOT NULL
     AND EXISTS (
       SELECT 1 FROM pg_constraint
        WHERE conname = 'ofertas_item_id_fkey'
          AND conrelid = 'stores.ofertas'::regclass
          AND confrelid = 'catalog.items'::regclass
     ) THEN
    ALTER TABLE stores.ofertas DROP CONSTRAINT ofertas_item_id_fkey;
    ALTER TABLE stores.ofertas
      ADD CONSTRAINT ofertas_item_id_fkey
      FOREIGN KEY (item_id) REFERENCES item.items(id) NOT VALID;
    ALTER TABLE stores.ofertas VALIDATE CONSTRAINT ofertas_item_id_fkey;
  END IF;
END $$;

ALTER TABLE item.outbox ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS outbox_admin ON item.outbox;
DROP POLICY IF EXISTS outbox_app ON item.outbox;
CREATE POLICY outbox_admin ON item.outbox
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY outbox_app ON item.outbox
  FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA item TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA item TO core_engine_app;
    GRANT SELECT ON item.items_vista TO core_engine_app;
  END IF;
END $$;

DO $$
DECLARE tabla text;
BEGIN
  FOREACH tabla IN ARRAY ARRAY[
    'items', 'item_atributo_esquemas',
    'item_imagenes', 'item_componentes', 'item_series', 'historico_precios'
  ] LOOP
    EXECUTE format('ALTER TABLE item.%I ENABLE ROW LEVEL SECURITY', tabla);
    EXECUTE format('DROP POLICY IF EXISTS %I ON item.%I', tabla || '_admin', tabla);
    EXECUTE format(
      'CREATE POLICY %I ON item.%I FOR ALL TO authenticated
         USING (auth.rol() IN (''super_admin'', ''admin''))
         WITH CHECK (auth.rol() IN (''super_admin'', ''admin''))',
      tabla || '_admin', tabla
    );
    EXECUTE format('DROP POLICY IF EXISTS %I ON item.%I', tabla || '_empresa', tabla);
    EXECUTE format(
      'CREATE POLICY %I ON item.%I FOR ALL TO authenticated
         USING (auth.rol() = ''empresario'' AND empresa_id::text = (auth.jwt() ->> ''tenant_id''))
         WITH CHECK (auth.rol() = ''empresario'' AND empresa_id::text = (auth.jwt() ->> ''tenant_id''))',
      tabla || '_empresa', tabla
    );
  END LOOP;
END $$;
