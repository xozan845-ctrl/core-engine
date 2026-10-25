-- ADR-28: productos con variantes e imágenes (migración aditiva, R-DB-6).
-- Variante = fila hija de catalog.productos (producto_padre_id). Conserva su
-- propio SKU/precio/stock, de modo que las ofertas, pedidos y reservas siguen
-- operando por (empresa_id, sku) sin abrir un stock paralelo.
BEGIN;

-- ── variantes ────────────────────────────────────────────────────────────
ALTER TABLE catalog.productos ADD COLUMN IF NOT EXISTS producto_padre_id uuid;
ALTER TABLE catalog.productos ADD COLUMN IF NOT EXISTS atributos jsonb NOT NULL DEFAULT '{}'::jsonb;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'productos_padre_fk'
  ) THEN
    ALTER TABLE catalog.productos
      ADD CONSTRAINT productos_padre_fk
      FOREIGN KEY (producto_padre_id) REFERENCES catalog.productos(id) ON DELETE CASCADE;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'productos_padre_no_self'
  ) THEN
    ALTER TABLE catalog.productos
      ADD CONSTRAINT productos_padre_no_self
      CHECK (producto_padre_id IS NULL OR producto_padre_id <> id);
  END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_productos_padre
  ON catalog.productos (empresa_id, producto_padre_id);

-- ── imágenes de producto/variante ────────────────────────────────────────
-- Máximo 3 por producto: orden 0..2 + UNIQUE(producto_id, orden). Una sola
-- principal por producto (índice parcial). objeto_id apunta a storage.objetos
-- sin FK cruzada entre servicios (R-DB-1); el tenant se guarda para aislar.
CREATE TABLE IF NOT EXISTS catalog.producto_imagenes (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id  uuid NOT NULL,
  producto_id uuid NOT NULL REFERENCES catalog.productos(id) ON DELETE CASCADE,
  objeto_id   uuid NOT NULL,
  orden       smallint NOT NULL CHECK (orden BETWEEN 0 AND 2),
  es_principal boolean NOT NULL DEFAULT false,
  creado_en   timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (producto_id, objeto_id),
  UNIQUE (producto_id, orden)
);
CREATE INDEX IF NOT EXISTS idx_producto_imagenes_empresa
  ON catalog.producto_imagenes (empresa_id, producto_id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_producto_imagen_principal
  ON catalog.producto_imagenes (producto_id) WHERE es_principal;

-- ── proyecciones: nuevas columnas ────────────────────────────────────────
ALTER TABLE catalog.producto_catalogo_vista ADD COLUMN IF NOT EXISTS producto_padre_id uuid;
ALTER TABLE catalog.producto_catalogo_vista ADD COLUMN IF NOT EXISTS atributos jsonb NOT NULL DEFAULT '{}'::jsonb;
UPDATE catalog.producto_catalogo_vista v
   SET producto_padre_id = p.producto_padre_id,
       atributos = p.atributos
  FROM catalog.productos p
 WHERE p.id = v.id;

-- ── agregado del padre ───────────────────────────────────────────────────
-- El padre no es vendible si tiene hijas: su stock/precio son solo la vista
-- agregada (suma de stock, mínimo precio). El servicio de ofertas rechaza el
-- SKU del padre cuando tiene variantes.
CREATE OR REPLACE FUNCTION catalog.fn_recalcular_padre(p_padre uuid)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  v_stock int;
  v_precio int;
  v_hay boolean;
BEGIN
  SELECT EXISTS (SELECT 1 FROM catalog.productos WHERE producto_padre_id = p_padre)
    INTO v_hay;
  IF NOT v_hay THEN
    RETURN;
  END IF;
  SELECT COALESCE(SUM(stock), 0), COALESCE(MIN(precio_base_cents), 0)
    INTO v_stock, v_precio
    FROM catalog.productos
   WHERE producto_padre_id = p_padre;
  UPDATE catalog.productos
     SET stock = v_stock,
         precio_base_cents = v_precio,
         estado = CASE WHEN v_stock > 0 THEN 'disponible' ELSE 'agotado' END
   WHERE id = p_padre;
END;
$$;

-- ── inventario: excluir la fila contenedora ──────────────────────────────
CREATE OR REPLACE FUNCTION catalog.fn_refrescar_mes_inventario(p_mes date)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  DELETE FROM catalog.inventario_mensual_mv WHERE periodo = p_mes;
  INSERT INTO catalog.inventario_mensual_mv
    (empresa_id, sku, nombre, categoria, periodo, stock_fisico, valor_stock_cents, actualizado_en)
  SELECT p.empresa_id, p.sku, p.nombre, p.categoria, p_mes,
         p.stock, p.stock * p.precio_base_cents, NOW()
  FROM catalog.productos p
  WHERE date_trunc('month', p.creado_en)::date <= p_mes
    AND NOT EXISTS (
      SELECT 1 FROM catalog.productos v WHERE v.producto_padre_id = p.id
    );
END;
$$;

-- ── trigger de producto: INSERT/UPDATE/DELETE + agregado del padre ────────
CREATE OR REPLACE FUNCTION catalog.fn_trigger_producto() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
  v_reservado int := 0;
BEGIN
  IF TG_OP = 'DELETE' THEN
    DELETE FROM catalog.producto_catalogo_vista WHERE id = OLD.id;
    DELETE FROM catalog.stock_vista
     WHERE empresa_id = OLD.empresa_id AND LOWER(sku) = LOWER(OLD.sku);
    IF OLD.producto_padre_id IS NOT NULL THEN
      PERFORM catalog.fn_recalcular_padre(OLD.producto_padre_id);
    END IF;
    RETURN NULL;
  END IF;

  SELECT COALESCE(SUM((item->>'cantidad')::int), 0) INTO v_reservado
  FROM catalog.reservas_ordenes ro,
       jsonb_array_elements(ro.items_json) AS item
  WHERE LOWER(item->>'sku') = LOWER(COALESCE(NEW.sku, OLD.sku))
    AND COALESCE(item->>'empresa_id', '00000000-0000-0000-0000-000000000000')
        = COALESCE(NEW.empresa_id, OLD.empresa_id)::text
    AND ro.creado_en > NOW() - INTERVAL '30 minutes';

  IF TG_OP = 'INSERT' THEN
    INSERT INTO catalog.producto_catalogo_vista
      (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents,
       disponible, estado, producto_padre_id, atributos, creado_en, actualizado_en)
    VALUES (NEW.id, NEW.empresa_id, NEW.sku, NEW.nombre, NEW.descripcion,
            NEW.categoria, NEW.precio_base_cents, NEW.stock, NEW.estado,
            NEW.producto_padre_id, NEW.atributos, NEW.creado_en, NOW());
  ELSE
    UPDATE catalog.producto_catalogo_vista
       SET empresa_id = NEW.empresa_id, nombre = NEW.nombre,
           descripcion = NEW.descripcion, categoria = NEW.categoria,
           precio_base_cents = NEW.precio_base_cents,
           disponible = NEW.stock, estado = NEW.estado,
           producto_padre_id = NEW.producto_padre_id,
           atributos = NEW.atributos, actualizado_en = NOW()
     WHERE id = NEW.id;
  END IF;

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

  PERFORM catalog.fn_refrescar_mes_inventario(date_trunc('month', NEW.creado_en)::date);

  IF NEW.producto_padre_id IS NOT NULL THEN
    PERFORM catalog.fn_recalcular_padre(NEW.producto_padre_id);
  ELSIF TG_OP = 'UPDATE' AND OLD.producto_padre_id IS NOT NULL
        AND NEW.producto_padre_id IS NULL THEN
    PERFORM catalog.fn_recalcular_padre(OLD.producto_padre_id);
  END IF;
  RETURN NULL;
END;
$$;

DROP TRIGGER IF EXISTS productos_proyeccion_trigger ON catalog.productos;
CREATE TRIGGER productos_proyeccion_trigger
AFTER INSERT OR UPDATE OR DELETE ON catalog.productos
FOR EACH ROW EXECUTE FUNCTION catalog.fn_trigger_producto();

-- ── bootstrap: conserva las columnas nuevas ─────────────────────────────
CREATE OR REPLACE FUNCTION catalog.fn_bootstrap_proyecciones()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  mes DATE;
BEGIN
  DELETE FROM catalog.producto_catalogo_vista;
  INSERT INTO catalog.producto_catalogo_vista
    (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents,
     disponible, estado, producto_padre_id, atributos, creado_en, actualizado_en)
  SELECT p.id, p.empresa_id, p.sku, p.nombre, p.descripcion, p.categoria,
         p.precio_base_cents, p.stock, p.estado, p.producto_padre_id,
         p.atributos, p.creado_en, NOW()
  FROM catalog.productos p;

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

  FOR mes IN
    SELECT DISTINCT date_trunc('month', creado_en)::date FROM catalog.productos
  LOOP
    PERFORM catalog.fn_refrescar_mes_inventario(mes);
  END LOOP;
END;
$$;

SELECT catalog.fn_bootstrap_proyecciones();

COMMIT;
