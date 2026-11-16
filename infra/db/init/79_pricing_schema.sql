-- ADR-0018 · corte de precios a su propio esquema.
--
-- Copia listas, precios, tarifas y promociones desde `catalog` a `pricing` sin
-- romper el rollback. `catalog.items` sigue siendo el dato maestro del ítem y
-- conserva `precio_cents` como precio base transitorio hasta `item-service`.

CREATE SCHEMA IF NOT EXISTS pricing;

CREATE TABLE IF NOT EXISTS pricing.listas_precio
  (LIKE catalog.listas_precio INCLUDING ALL);
INSERT INTO pricing.listas_precio
SELECT * FROM catalog.listas_precio
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS pricing.lista_precio_items
  (LIKE catalog.lista_precio_items INCLUDING ALL);
INSERT INTO pricing.lista_precio_items
SELECT * FROM catalog.lista_precio_items
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS pricing.item_tarifas
  (LIKE catalog.item_tarifas INCLUDING ALL);
INSERT INTO pricing.item_tarifas
SELECT * FROM catalog.item_tarifas
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS pricing.promociones
  (LIKE catalog.promociones INCLUDING ALL);
INSERT INTO pricing.promociones
SELECT * FROM catalog.promociones
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS pricing.outbox
  (LIKE catalog.outbox INCLUDING ALL);
INSERT INTO pricing.outbox
SELECT * FROM catalog.outbox
ON CONFLICT (event_id) DO NOTHING;

-- Claves foráneas que `LIKE` no copia.
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint
     WHERE conname = 'pricing_lista_precio_items_lista_fk'
       AND conrelid = 'pricing.lista_precio_items'::regclass
  ) THEN
    ALTER TABLE pricing.lista_precio_items
      ADD CONSTRAINT pricing_lista_precio_items_lista_fk
      FOREIGN KEY (lista_id) REFERENCES pricing.listas_precio(id)
      ON DELETE CASCADE NOT VALID;
  END IF;
END $$;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA pricing TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA pricing TO core_engine_app;
  END IF;
END $$;

DO $$
DECLARE tabla text;
BEGIN
  FOREACH tabla IN ARRAY ARRAY['listas_precio', 'lista_precio_items', 'item_tarifas', 'promociones'] LOOP
    EXECUTE format('ALTER TABLE pricing.%I ENABLE ROW LEVEL SECURITY', tabla);
    EXECUTE format('DROP POLICY IF EXISTS %I ON pricing.%I', tabla || '_admin', tabla);
    EXECUTE format(
      'CREATE POLICY %I ON pricing.%I FOR ALL TO authenticated
         USING (auth.rol() IN (''super_admin'', ''admin''))
         WITH CHECK (auth.rol() IN (''super_admin'', ''admin''))',
      tabla || '_admin', tabla
    );
    EXECUTE format('DROP POLICY IF EXISTS %I ON pricing.%I', tabla || '_empresa', tabla);
    EXECUTE format(
      'CREATE POLICY %I ON pricing.%I FOR ALL TO authenticated
         USING (auth.rol() = ''empresario'' AND empresa_id::text = (auth.jwt() ->> ''tenant_id''))
         WITH CHECK (auth.rol() = ''empresario'' AND empresa_id::text = (auth.jwt() ->> ''tenant_id''))',
      tabla || '_empresa', tabla
    );
  END LOOP;
END $$;

-- El poller del Outbox corre sin claims de usuario.
ALTER TABLE pricing.outbox ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS outbox_admin ON pricing.outbox;
DROP POLICY IF EXISTS outbox_app ON pricing.outbox;
CREATE POLICY outbox_admin ON pricing.outbox
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY outbox_app ON pricing.outbox
  FOR ALL TO core_engine_app
  USING (true)
  WITH CHECK (true);
