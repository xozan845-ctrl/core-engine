-- Catálogo operativo (R-PS-01/R-PS-02/R-PS-03): metadatos del producto y
-- recurso de servicios. Migración aditiva e idempotente (R-DB-6).
--
-- `estado` sigue describiendo disponibilidad de stock (disponible/agotado) y
-- `estado_operativo` el alta/baja comercial (activo/inactivo). Separarlos evita
-- que un producto agotado desaparezca del panel del empresario.
BEGIN;

-- ── metadatos del producto ───────────────────────────────────────────────
ALTER TABLE catalog.productos
  ADD COLUMN IF NOT EXISTS costo_cents integer NOT NULL DEFAULT 0 CHECK (costo_cents >= 0),
  ADD COLUMN IF NOT EXISTS marca text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS codigo_barras text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS unidad text NOT NULL DEFAULT 'Unidad (u)',
  ADD COLUMN IF NOT EXISTS permite_fracciones boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS unidad_compra text NOT NULL DEFAULT 'Igual que la de venta',
  ADD COLUMN IF NOT EXISTS contenido_cantidad integer NOT NULL DEFAULT 0 CHECK (contenido_cantidad >= 0),
  ADD COLUMN IF NOT EXISTS contenido_unidad text NOT NULL DEFAULT 'Sin contenido',
  ADD COLUMN IF NOT EXISTS tipo_precio text NOT NULL DEFAULT 'fijo' CHECK (tipo_precio IN ('fijo', 'escala')),
  ADD COLUMN IF NOT EXISTS tipo_costo text NOT NULL DEFAULT 'fijo' CHECK (tipo_costo IN ('fijo', 'escala')),
  ADD COLUMN IF NOT EXISTS stock_minimo integer NOT NULL DEFAULT 0 CHECK (stock_minimo >= 0),
  ADD COLUMN IF NOT EXISTS stock_maximo integer CHECK (stock_maximo IS NULL OR stock_maximo >= 0),
  ADD COLUMN IF NOT EXISTS garantia_meses integer CHECK (garantia_meses IS NULL OR garantia_meses >= 0),
  ADD COLUMN IF NOT EXISTS es_materia_prima boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS ubicacion text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS estado_operativo text NOT NULL DEFAULT 'activo'
    CHECK (estado_operativo IN ('activo', 'inactivo')),
  ADD COLUMN IF NOT EXISTS actualizado_en timestamptz NOT NULL DEFAULT NOW();

CREATE INDEX IF NOT EXISTS idx_productos_empresa_estado_operativo
  ON catalog.productos (empresa_id, estado_operativo);
CREATE UNIQUE INDEX IF NOT EXISTS ux_productos_empresa_barcode
  ON catalog.productos (empresa_id, codigo_barras)
  WHERE codigo_barras <> '';

-- ── proyección de lectura ────────────────────────────────────────────────
ALTER TABLE catalog.producto_catalogo_vista
  ADD COLUMN IF NOT EXISTS costo_cents integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS marca text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS codigo_barras text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS unidad text NOT NULL DEFAULT 'Unidad (u)',
  ADD COLUMN IF NOT EXISTS permite_fracciones boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS unidad_compra text NOT NULL DEFAULT 'Igual que la de venta',
  ADD COLUMN IF NOT EXISTS contenido_cantidad integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS contenido_unidad text NOT NULL DEFAULT 'Sin contenido',
  ADD COLUMN IF NOT EXISTS tipo_precio text NOT NULL DEFAULT 'fijo',
  ADD COLUMN IF NOT EXISTS tipo_costo text NOT NULL DEFAULT 'fijo',
  ADD COLUMN IF NOT EXISTS stock_minimo integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS stock_maximo integer,
  ADD COLUMN IF NOT EXISTS garantia_meses integer,
  ADD COLUMN IF NOT EXISTS es_materia_prima boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS ubicacion text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS estado_operativo text NOT NULL DEFAULT 'activo';

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
       costo_cents, marca, codigo_barras, unidad, permite_fracciones,
       unidad_compra, contenido_cantidad, contenido_unidad, tipo_precio,
       tipo_costo, stock_minimo, stock_maximo, garantia_meses,
       es_materia_prima, ubicacion, estado_operativo,
       disponible, estado, producto_padre_id, atributos, creado_en, actualizado_en)
    VALUES (NEW.id, NEW.empresa_id, NEW.sku, NEW.nombre, NEW.descripcion,
            NEW.categoria, NEW.precio_base_cents, NEW.costo_cents, NEW.marca,
            NEW.codigo_barras, NEW.unidad, NEW.permite_fracciones,
            NEW.unidad_compra, NEW.contenido_cantidad, NEW.contenido_unidad,
            NEW.tipo_precio, NEW.tipo_costo, NEW.stock_minimo, NEW.stock_maximo,
            NEW.garantia_meses, NEW.es_materia_prima, NEW.ubicacion,
            NEW.estado_operativo, NEW.stock, NEW.estado, NEW.producto_padre_id,
            NEW.atributos, NEW.creado_en, NOW());
  ELSE
    UPDATE catalog.producto_catalogo_vista
       SET empresa_id = NEW.empresa_id, nombre = NEW.nombre,
           descripcion = NEW.descripcion, categoria = NEW.categoria,
           precio_base_cents = NEW.precio_base_cents,
           costo_cents = NEW.costo_cents, marca = NEW.marca,
           codigo_barras = NEW.codigo_barras, unidad = NEW.unidad,
           permite_fracciones = NEW.permite_fracciones,
           unidad_compra = NEW.unidad_compra,
           contenido_cantidad = NEW.contenido_cantidad,
           contenido_unidad = NEW.contenido_unidad,
           tipo_precio = NEW.tipo_precio, tipo_costo = NEW.tipo_costo,
           stock_minimo = NEW.stock_minimo, stock_maximo = NEW.stock_maximo,
           garantia_meses = NEW.garantia_meses,
           es_materia_prima = NEW.es_materia_prima, ubicacion = NEW.ubicacion,
           estado_operativo = NEW.estado_operativo,
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

CREATE OR REPLACE FUNCTION catalog.fn_bootstrap_proyecciones()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  mes DATE;
BEGIN
  DELETE FROM catalog.producto_catalogo_vista;
  INSERT INTO catalog.producto_catalogo_vista
    (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents,
     costo_cents, marca, codigo_barras, unidad, permite_fracciones,
     unidad_compra, contenido_cantidad, contenido_unidad, tipo_precio,
     tipo_costo, stock_minimo, stock_maximo, garantia_meses,
     es_materia_prima, ubicacion, estado_operativo,
     disponible, estado, producto_padre_id, atributos, creado_en, actualizado_en)
  SELECT p.id, p.empresa_id, p.sku, p.nombre, p.descripcion, p.categoria,
         p.precio_base_cents, p.costo_cents, p.marca, p.codigo_barras, p.unidad,
         p.permite_fracciones, p.unidad_compra, p.contenido_cantidad,
         p.contenido_unidad, p.tipo_precio, p.tipo_costo, p.stock_minimo,
         p.stock_maximo, p.garantia_meses, p.es_materia_prima, p.ubicacion,
         p.estado_operativo, p.stock, p.estado, p.producto_padre_id,
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

-- ── servicios de la empresa (R-PS-02) ────────────────────────────────────
CREATE TABLE IF NOT EXISTS catalog.servicios (
  id                 uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id         uuid NOT NULL,
  codigo             text NOT NULL,
  nombre             text NOT NULL,
  descripcion        text NOT NULL DEFAULT '',
  categoria          text NOT NULL DEFAULT '',
  marca              text NOT NULL DEFAULT '',
  codigo_barras      text NOT NULL DEFAULT '',
  precio_cents       integer NOT NULL CHECK (precio_cents >= 0),
  costo_cents        integer NOT NULL DEFAULT 0 CHECK (costo_cents >= 0),
  unidad             text NOT NULL DEFAULT 'Hora (h)',
  duracion_minutos   integer NOT NULL CHECK (duracion_minutos BETWEEN 1 AND 1440),
  estado             text NOT NULL DEFAULT 'activo' CHECK (estado IN ('activo', 'inactivo')),
  creado_en          timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en     timestamptz NOT NULL DEFAULT NOW(),
  eliminado_en       timestamptz
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_servicios_empresa_codigo
  ON catalog.servicios (empresa_id, upper(codigo))
  WHERE eliminado_en IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS ux_servicios_empresa_barcode
  ON catalog.servicios (empresa_id, codigo_barras)
  WHERE codigo_barras <> '' AND eliminado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_servicios_empresa_estado
  ON catalog.servicios (empresa_id, estado, creado_en DESC)
  WHERE eliminado_en IS NULL;

COMMIT;
