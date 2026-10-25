-- ADR-27: catálogo por empresa.
-- Migración aditiva: cada empresa tiene su propio catálogo, con SKU único por
-- empresa. Los productos legados sin empresa pasan a una empresa comodín
-- (inactiva) y quedan fuera de los listados y de la publicación en tiendas.
-- La empresa de logística queda excluida del catálogo por su tipo_empresa.
BEGIN;

-- ── identity: tipo de empresa ────────────────────────────────────────────
-- 'comercial' = vende; 'logistica' = transporta (sin catálogo);
-- 'plataforma' = empresa técnica/comodín (sin catálogo público).
ALTER TABLE identity.empresas ADD COLUMN IF NOT EXISTS tipo_empresa text;
UPDATE identity.empresas SET tipo_empresa = 'comercial' WHERE tipo_empresa IS NULL;
ALTER TABLE identity.empresas ALTER COLUMN tipo_empresa SET DEFAULT 'comercial';
ALTER TABLE identity.empresas ALTER COLUMN tipo_empresa SET NOT NULL;
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'empresas_tipo_empresa_check'
  ) THEN
    ALTER TABLE identity.empresas ADD CONSTRAINT empresas_tipo_empresa_check
      CHECK (tipo_empresa IN ('comercial', 'logistica', 'plataforma'));
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS idx_empresas_tipo ON identity.empresas (tipo_empresa);

-- Empresa comodín para el catálogo legado. Solo se crea si ya hay usuarios
-- (creado_por es NOT NULL); en una base vacía no hay productos que migrar.
INSERT INTO identity.empresas
  (id, nombre, slug, modalidad_logistica, estado, creado_por, tipo_empresa)
SELECT
  '00000000-0000-0000-0000-000000000000',
  'Catálogo legado',
  'catalogo-legado',
  'plataforma',
  'inactiva',
  (SELECT id FROM identity.usuarios ORDER BY creado_en ASC, id ASC LIMIT 1),
  'plataforma'
WHERE EXISTS (SELECT 1 FROM identity.usuarios)
  AND NOT EXISTS (
    SELECT 1 FROM identity.empresas
    WHERE id = '00000000-0000-0000-0000-000000000000'
  )
ON CONFLICT DO NOTHING;

-- ── catalog.productos: empresa obligatoria y SKU por empresa ──────────────
UPDATE catalog.productos
   SET empresa_id = '00000000-0000-0000-0000-000000000000'
 WHERE empresa_id IS NULL;
ALTER TABLE catalog.productos ALTER COLUMN empresa_id SET NOT NULL;
CREATE INDEX IF NOT EXISTS idx_productos_empresa ON catalog.productos (empresa_id);
CREATE INDEX IF NOT EXISTS idx_productos_empresa_estado ON catalog.productos (empresa_id, estado);

-- El SKU deja de ser único global: ahora identifica dentro de una empresa.
-- Primero se retiran las FK que dependen del índice único global.
ALTER TABLE catalog.historico_precios
  DROP CONSTRAINT IF EXISTS historico_precios_producto_sku_fkey;
ALTER TABLE catalog.ajustes_stock
  DROP CONSTRAINT IF EXISTS ajustes_stock_producto_sku_fkey;
ALTER TABLE catalog.productos DROP CONSTRAINT IF EXISTS productos_sku_key;
DROP INDEX IF EXISTS catalog.productos_sku_key;
CREATE UNIQUE INDEX IF NOT EXISTS ux_productos_empresa_sku
  ON catalog.productos (empresa_id, sku);

-- ── histórico y ajustes: la referencia pasa a ser (empresa, sku) ──────────
ALTER TABLE catalog.historico_precios ADD COLUMN IF NOT EXISTS empresa_id uuid;
UPDATE catalog.historico_precios h
   SET empresa_id = p.empresa_id
  FROM catalog.productos p
 WHERE p.sku = h.producto_sku
   AND h.empresa_id IS NULL;
UPDATE catalog.historico_precios
   SET empresa_id = '00000000-0000-0000-0000-000000000000'
 WHERE empresa_id IS NULL;
ALTER TABLE catalog.historico_precios ALTER COLUMN empresa_id SET NOT NULL;
CREATE INDEX IF NOT EXISTS idx_historico_precios_empresa_sku
  ON catalog.historico_precios (empresa_id, producto_sku, vigente_desde DESC);

ALTER TABLE catalog.ajustes_stock ADD COLUMN IF NOT EXISTS empresa_id uuid;
UPDATE catalog.ajustes_stock a
   SET empresa_id = p.empresa_id
  FROM catalog.productos p
 WHERE p.sku = a.producto_sku
   AND a.empresa_id IS NULL;
UPDATE catalog.ajustes_stock
   SET empresa_id = '00000000-0000-0000-0000-000000000000'
 WHERE empresa_id IS NULL;
ALTER TABLE catalog.ajustes_stock ALTER COLUMN empresa_id SET NOT NULL;
CREATE INDEX IF NOT EXISTS idx_ajustes_stock_empresa_sku
  ON catalog.ajustes_stock (empresa_id, producto_sku, realizado_en DESC);

-- ── proyecciones de lectura ──────────────────────────────────────────────
ALTER TABLE catalog.producto_catalogo_vista ADD COLUMN IF NOT EXISTS empresa_id uuid;
UPDATE catalog.producto_catalogo_vista v
   SET empresa_id = p.empresa_id
  FROM catalog.productos p
 WHERE p.id = v.id
   AND v.empresa_id IS NULL;
UPDATE catalog.producto_catalogo_vista
   SET empresa_id = '00000000-0000-0000-0000-000000000000'
 WHERE empresa_id IS NULL;
ALTER TABLE catalog.producto_catalogo_vista ALTER COLUMN empresa_id SET NOT NULL;
ALTER TABLE catalog.producto_catalogo_vista
  DROP CONSTRAINT IF EXISTS producto_catalogo_vista_sku_key;
CREATE UNIQUE INDEX IF NOT EXISTS ux_catalogo_vista_empresa_sku
  ON catalog.producto_catalogo_vista (empresa_id, sku);
CREATE INDEX IF NOT EXISTS idx_catalogo_vista_empresa
  ON catalog.producto_catalogo_vista (empresa_id, estado);

ALTER TABLE catalog.stock_vista ADD COLUMN IF NOT EXISTS empresa_id uuid;
UPDATE catalog.stock_vista s
   SET empresa_id = p.empresa_id
  FROM catalog.productos p
 WHERE p.sku = s.sku
   AND s.empresa_id IS NULL;
UPDATE catalog.stock_vista
   SET empresa_id = '00000000-0000-0000-0000-000000000000'
 WHERE empresa_id IS NULL;
ALTER TABLE catalog.stock_vista ALTER COLUMN empresa_id SET NOT NULL;
ALTER TABLE catalog.stock_vista DROP CONSTRAINT IF EXISTS stock_vista_pkey;
ALTER TABLE catalog.stock_vista
  ADD CONSTRAINT stock_vista_pkey PRIMARY KEY (empresa_id, sku);

ALTER TABLE catalog.inventario_mensual_mv ADD COLUMN IF NOT EXISTS empresa_id uuid;
UPDATE catalog.inventario_mensual_mv i
   SET empresa_id = p.empresa_id
  FROM catalog.productos p
 WHERE p.sku = i.sku
   AND i.empresa_id IS NULL;
UPDATE catalog.inventario_mensual_mv
   SET empresa_id = '00000000-0000-0000-0000-000000000000'
 WHERE empresa_id IS NULL;
ALTER TABLE catalog.inventario_mensual_mv ALTER COLUMN empresa_id SET NOT NULL;
ALTER TABLE catalog.inventario_mensual_mv DROP CONSTRAINT IF EXISTS inventario_mensual_mv_pkey;
ALTER TABLE catalog.inventario_mensual_mv
  ADD CONSTRAINT inventario_mensual_mv_pkey PRIMARY KEY (empresa_id, sku, periodo);

-- ── funciones de proyección (redefinidas con empresa) ────────────────────
CREATE OR REPLACE FUNCTION catalog.fn_refrescar_mes_inventario(p_mes date)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  DELETE FROM catalog.inventario_mensual_mv WHERE periodo = p_mes;
  INSERT INTO catalog.inventario_mensual_mv
    (empresa_id, sku, nombre, categoria, periodo, stock_fisico, valor_stock_cents, actualizado_en)
  SELECT p.empresa_id, p.sku, p.nombre, p.categoria, p_mes,
         p.stock, p.stock * p.precio_base_cents, NOW()
  FROM catalog.productos p
  WHERE date_trunc('month', p.creado_en)::date <= p_mes;  -- productos existentes al mes
END;
$$;

CREATE OR REPLACE FUNCTION catalog.fn_bootstrap_proyecciones()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  mes DATE;
BEGIN
  -- 1) producto_catalogo_vista
  DELETE FROM catalog.producto_catalogo_vista;
  INSERT INTO catalog.producto_catalogo_vista
    (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents,
     disponible, estado, creado_en, actualizado_en)
  SELECT p.id, p.empresa_id, p.sku, p.nombre, p.descripcion, p.categoria,
         p.precio_base_cents, p.stock, p.estado, p.creado_en, NOW()
  FROM catalog.productos p;

  -- 2) stock_vista (reservas agrupadas por empresa y SKU)
  DELETE FROM catalog.stock_vista;
  INSERT INTO catalog.stock_vista
    (empresa_id, sku, nombre, stock_fisico, reservado, disponible,
     precio_base_cents, estado, actualizado_en)
  SELECT p.empresa_id, p.sku, p.nombre, p.stock + COALESCE(r.reservado, 0),
         COALESCE(r.reservado, 0), p.stock, p.precio_base_cents, p.estado, NOW()
  FROM catalog.productos p
  LEFT JOIN (
    SELECT COALESCE(item->>'empresa_id', '00000000-0000-0000-0000-000000000000') AS empresa_id,
           LOWER(item->>'sku') AS sku,
           SUM((item->>'cantidad')::int) AS reservado
    FROM catalog.reservas_ordenes ro,
         jsonb_array_elements(ro.items_json) AS item
    WHERE ro.creado_en > NOW() - INTERVAL '30 minutes'
    GROUP BY 1, 2
  ) r ON r.empresa_id = p.empresa_id::text AND r.sku = LOWER(p.sku);

  -- 3) inventario_mensual_mv por cada mes distinto en productos
  FOR mes IN
    SELECT DISTINCT date_trunc('month', creado_en)::date FROM catalog.productos
  LOOP
    PERFORM catalog.fn_refrescar_mes_inventario(mes);
  END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION catalog.fn_trigger_producto() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
  v_reservado int := 0;
BEGIN
  IF TG_OP IN ('INSERT', 'UPDATE') THEN
    -- reservas vigentes del mismo producto (empresa + SKU)
    SELECT COALESCE(SUM((item->>'cantidad')::int), 0) INTO v_reservado
    FROM catalog.reservas_ordenes ro,
         jsonb_array_elements(ro.items_json) AS item
    WHERE LOWER(item->>'sku') = LOWER(COALESCE(NEW.sku, OLD.sku))
      AND COALESCE(item->>'empresa_id', '00000000-0000-0000-0000-000000000000')
          = COALESCE(NEW.empresa_id, OLD.empresa_id)::text
      AND ro.creado_en > NOW() - INTERVAL '30 minutes';

    -- producto_catalogo_vista (la clave es id, que no cambia)
    IF TG_OP = 'INSERT' THEN
      INSERT INTO catalog.producto_catalogo_vista
        (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents,
         disponible, estado, creado_en, actualizado_en)
      VALUES (NEW.id, NEW.empresa_id, NEW.sku, NEW.nombre, NEW.descripcion,
              NEW.categoria, NEW.precio_base_cents, NEW.stock, NEW.estado,
              NEW.creado_en, NOW());
    ELSE
      UPDATE catalog.producto_catalogo_vista
         SET empresa_id = NEW.empresa_id, nombre = NEW.nombre,
             descripcion = NEW.descripcion, categoria = NEW.categoria,
             precio_base_cents = NEW.precio_base_cents,
             disponible = NEW.stock, estado = NEW.estado, actualizado_en = NOW()
       WHERE id = NEW.id;
    END IF;

    -- stock_vista: si cambia la clave lógica se retira la fila anterior
    IF TG_OP = 'UPDATE'
       AND (NEW.empresa_id IS DISTINCT FROM OLD.empresa_id OR NEW.sku IS DISTINCT FROM OLD.sku) THEN
      DELETE FROM catalog.stock_vista
       WHERE empresa_id = OLD.empresa_id AND sku = OLD.sku;
    END IF;
    INSERT INTO catalog.stock_vista
      (empresa_id, sku, nombre, stock_fisico, reservado, disponible,
       precio_base_cents, estado, actualizado_en)
    VALUES (NEW.empresa_id, NEW.sku, NEW.nombre, NEW.stock + v_reservado,
            v_reservado, NEW.stock, NEW.precio_base_cents, NEW.estado, NOW())
    ON CONFLICT (empresa_id, sku) DO UPDATE SET
      nombre = EXCLUDED.nombre,
      stock_fisico = EXCLUDED.stock_fisico,
      reservado = EXCLUDED.reservado,
      disponible = EXCLUDED.disponible,
      precio_base_cents = EXCLUDED.precio_base_cents,
      estado = EXCLUDED.estado,
      actualizado_en = NOW();

    -- inventario_mensual_mv del mes del producto
    PERFORM catalog.fn_refrescar_mes_inventario(date_trunc('month', NEW.creado_en)::date);
  END IF;
  RETURN NULL;
END;
$$;

-- Recalcula el reservado por (empresa, SKU). Sin este ámbito, una reserva de
-- una empresa actualizaba la proyección de todas las empresas con ese SKU.
CREATE OR REPLACE FUNCTION catalog.fn_trigger_reserva() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
  v_items jsonb := '[]'::jsonb;
BEGIN
  IF TG_OP IN ('DELETE', 'UPDATE') THEN
    v_items := OLD.items_json;
  END IF;
  IF TG_OP IN ('INSERT', 'UPDATE') THEN
    v_items := v_items || NEW.items_json;
  END IF;

  WITH claves AS (
    SELECT DISTINCT
      COALESCE(item->>'empresa_id', '00000000-0000-0000-0000-000000000000')::uuid AS empresa_id,
      LOWER(item->>'sku') AS sku
    FROM jsonb_array_elements(v_items) AS item
  ),
  reservado AS (
    SELECT COALESCE(item->>'empresa_id', '00000000-0000-0000-0000-000000000000')::uuid AS empresa_id,
           LOWER(item->>'sku') AS sku,
           SUM((item->>'cantidad')::int)::int AS cantidad
    FROM catalog.reservas_ordenes ro,
         jsonb_array_elements(ro.items_json) AS item
    WHERE ro.creado_en > NOW() - INTERVAL '30 minutes'
    GROUP BY 1, 2
  )
  UPDATE catalog.stock_vista s
     SET reservado = COALESCE(r.cantidad, 0),
         stock_fisico = p.stock + COALESCE(r.cantidad, 0),
         disponible = p.stock,
         actualizado_en = NOW()
    FROM claves c
    JOIN catalog.productos p
      ON p.empresa_id = c.empresa_id AND LOWER(p.sku) = c.sku
    LEFT JOIN reservado r
      ON r.empresa_id = c.empresa_id AND r.sku = c.sku
   WHERE s.empresa_id = c.empresa_id AND LOWER(s.sku) = c.sku;

  RETURN NULL;
END;
$$;

-- Reconstruye las proyecciones con la nueva clave por empresa.
SELECT catalog.fn_bootstrap_proyecciones();

-- ── inteligencia: la demanda de un SKU ya no es global ───────────────────
-- Se aplica en 03a_metricas_producto_empresa.sql: este archivo corre antes de
-- que 03_intelligence.sql cree el esquema intelligence.

COMMIT;
