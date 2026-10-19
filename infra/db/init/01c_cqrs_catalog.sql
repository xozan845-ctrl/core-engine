-- CQRS lógico (ADR-07): proyecciones de lectura para Catalog/Inventario. Idempotente.
-- Se ejecuta despues de 01_esquemas (orden alfabetico) tanto en fresh como en DB viva.
CREATE SCHEMA IF NOT EXISTS catalog;

-- 1) Vista de catálogo público (read-optimized para listados/búsqueda)
CREATE TABLE IF NOT EXISTS catalog.producto_catalogo_vista (
  id              uuid PRIMARY KEY,
  empresa_id      uuid NOT NULL,
  sku             text NOT NULL UNIQUE,
  nombre          text NOT NULL,
  descripcion     text,
  categoria       text NOT NULL,
  precio_base_cents integer NOT NULL,
  disponible      integer NOT NULL,          -- catalog.productos.stock ya descuenta reservas
  estado          text NOT NULL,             -- 'disponible' | 'agotado'
  creado_en       timestamptz NOT NULL,
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);
ALTER TABLE catalog.producto_catalogo_vista ADD COLUMN IF NOT EXISTS empresa_id uuid;
CREATE INDEX IF NOT EXISTS idx_catalogo_categoria ON catalog.producto_catalogo_vista (categoria);
CREATE INDEX IF NOT EXISTS idx_catalogo_estado ON catalog.producto_catalogo_vista (estado);
-- pg_trgm no está garantizado; usar ILIKE o crear extensión si se necesita búsqueda full-text
-- CREATE INDEX IF NOT EXISTS idx_catalogo_nombre_trgm ON catalog.producto_catalogo_vista USING gin (nombre gin_trgm_ops);

-- 2) Vista de stock detallada (para admin, alertas, valorización)
CREATE TABLE IF NOT EXISTS catalog.stock_vista (
  sku                 text PRIMARY KEY,
  nombre              text NOT NULL,
  stock_fisico        integer NOT NULL DEFAULT 0, -- disponible + reservado vigente
  reservado           integer NOT NULL DEFAULT 0,      -- sum(items_json->>'cantidad') de reservas_ordenes vigentes
  disponible          integer NOT NULL DEFAULT 0,      -- stock_fisico - reservado
  en_transito         integer NOT NULL DEFAULT 0,      -- futuro: recepciones pendientes
  precio_base_cents   integer NOT NULL DEFAULT 0,
  valor_disponible_cents  integer GENERATED ALWAYS AS (disponible * precio_base_cents) STORED,
  valor_total_cents       integer GENERATED ALWAYS AS (stock_fisico * precio_base_cents) STORED,
  estado              text NOT NULL DEFAULT 'disponible',
  actualizado_en      timestamptz NOT NULL DEFAULT NOW()
);

-- 3) Materialized view: inventario valorizado por mes (para finanzas/comisiones)
CREATE TABLE IF NOT EXISTS catalog.inventario_mensual_mv (
  sku                text NOT NULL,
  nombre             text NOT NULL,
  categoria          text NOT NULL,
  periodo            date NOT NULL,               -- primer día del mes
  stock_fisico       integer NOT NULL DEFAULT 0,
  valor_stock_cents  integer NOT NULL DEFAULT 0,
  actualizado_en     timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (sku, periodo)
);
CREATE INDEX IF NOT EXISTS idx_inv_mensual_periodo ON catalog.inventario_mensual_mv (periodo);
CREATE INDEX IF NOT EXISTS idx_inv_mensual_categoria ON catalog.inventario_mensual_mv (categoria);

-- Función auxiliar: refresca un mes en inventario_mensual_mv
CREATE OR REPLACE FUNCTION catalog.fn_refrescar_mes_inventario(p_mes date)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  DELETE FROM catalog.inventario_mensual_mv WHERE periodo = p_mes;
  INSERT INTO catalog.inventario_mensual_mv (sku, nombre, categoria, periodo, stock_fisico, valor_stock_cents, actualizado_en)
  SELECT p.sku, p.nombre, p.categoria, p_mes, p.stock, p.stock * p.precio_base_cents, NOW()
  FROM catalog.productos p
  WHERE date_trunc('month', p.creado_en)::date <= p_mes;  -- productos existentes al mes
END;
$$;

-- Función de refresh completo (bootstrap)
CREATE OR REPLACE FUNCTION catalog.fn_bootstrap_proyecciones()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  mes DATE;
BEGIN
  -- 1) producto_catalogo_vista
  DELETE FROM catalog.producto_catalogo_vista;
  INSERT INTO catalog.producto_catalogo_vista (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents, disponible, estado, creado_en, actualizado_en)
  SELECT p.id, p.empresa_id, p.sku, p.nombre, p.descripcion, p.categoria, p.precio_base_cents,
         p.stock,
         p.estado, p.creado_en, NOW()
  FROM catalog.productos p;

  -- 2) stock_vista
  DELETE FROM catalog.stock_vista;
  INSERT INTO catalog.stock_vista (sku, nombre, stock_fisico, reservado, disponible, precio_base_cents, estado, actualizado_en)
  SELECT p.sku, p.nombre, p.stock + COALESCE(r.reservado, 0),
         COALESCE(r.reservado, 0),
         p.stock,
         p.precio_base_cents,
         p.estado, NOW()
  FROM catalog.productos p
  LEFT JOIN (
    SELECT LOWER(item->>'sku') AS sku, SUM((item->>'cantidad')::int) AS reservado
    FROM catalog.reservas_ordenes ro,
         jsonb_array_elements(ro.items_json) AS item
    WHERE ro.creado_en > NOW() - INTERVAL '30 minutes'
    GROUP BY LOWER(item->>'sku')
  ) r ON LOWER(p.sku) = r.sku;

  -- 3) inventario_mensual_mv por cada mes distinto en productos
  FOR mes IN
    SELECT DISTINCT date_trunc('month', creado_en)::date FROM catalog.productos
  LOOP
    PERFORM catalog.fn_refrescar_mes_inventario(mes);
  END LOOP;
END;
$$;

-- Trigger functions (retornan TRIGGER para poder usar RETURN NULL)
CREATE OR REPLACE FUNCTION catalog.fn_trigger_producto() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
  v_reservado int := 0;
BEGIN
  IF TG_OP IN ('INSERT', 'UPDATE') THEN
    -- recalcular reservado actual para este SKU
    SELECT COALESCE(SUM((item->>'cantidad')::int), 0) INTO v_reservado
    FROM catalog.reservas_ordenes ro,
         jsonb_array_elements(ro.items_json) AS item
    WHERE LOWER(item->>'sku') = LOWER(COALESCE(NEW.sku, OLD.sku))
      AND ro.creado_en > NOW() - INTERVAL '30 minutes';

    -- producto_catalogo_vista
    IF TG_OP = 'INSERT' THEN
      INSERT INTO catalog.producto_catalogo_vista (id, empresa_id, sku, nombre, descripcion, categoria, precio_base_cents, disponible, estado, creado_en, actualizado_en)
      VALUES (NEW.id, NEW.empresa_id, NEW.sku, NEW.nombre, NEW.descripcion, NEW.categoria, NEW.precio_base_cents,
              NEW.stock, NEW.estado, NEW.creado_en, NOW());
    ELSE
      UPDATE catalog.producto_catalogo_vista
      SET empresa_id = NEW.empresa_id, nombre = NEW.nombre, descripcion = NEW.descripcion, categoria = NEW.categoria,
          precio_base_cents = NEW.precio_base_cents,
          disponible = NEW.stock,
          estado = NEW.estado, actualizado_en = NOW()
      WHERE id = NEW.id;
    END IF;

    -- stock_vista
    IF TG_OP = 'INSERT' THEN
      INSERT INTO catalog.stock_vista (sku, nombre, stock_fisico, reservado, disponible, precio_base_cents, estado, actualizado_en)
      VALUES (NEW.sku, NEW.nombre, NEW.stock + v_reservado, v_reservado, NEW.stock, NEW.precio_base_cents, NEW.estado, NOW());
    ELSE
      UPDATE catalog.stock_vista
      SET nombre = NEW.nombre, stock_fisico = NEW.stock + v_reservado, reservado = v_reservado,
          disponible = NEW.stock,
          precio_base_cents = NEW.precio_base_cents, estado = NEW.estado, actualizado_en = NOW()
      WHERE sku = NEW.sku;
    END IF;

    -- inventario_mensual_mv del mes del producto
    PERFORM catalog.fn_refrescar_mes_inventario(date_trunc('month', NEW.creado_en)::date);
  END IF;
  RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION catalog.fn_trigger_reserva() RETURNS TRIGGER LANGUAGE plpgsql AS $$
DECLARE
  v_sku text;
  v_items jsonb := '[]'::jsonb;
BEGIN
  IF TG_OP IN ('DELETE', 'UPDATE') THEN
    v_items := OLD.items_json;
  END IF;
  IF TG_OP IN ('INSERT', 'UPDATE') THEN
    v_items := v_items || NEW.items_json;
  END IF;

  -- Recalcular desde la fuente evita doble descuento y deriva correctamente
  -- los cambios de cantidad/SKU en UPDATE. La reserva no modifica disponible:
  -- catalog.productos.stock ya fue decrementado en la misma transaccion.
  FOR v_sku IN
    SELECT DISTINCT LOWER(item->>'sku') FROM jsonb_array_elements(v_items) AS item
  LOOP
    UPDATE catalog.stock_vista s
    SET reservado = r.reservado,
        stock_fisico = p.stock + r.reservado,
        disponible = p.stock,
        actualizado_en = NOW()
    FROM catalog.productos p,
         LATERAL (
           SELECT COALESCE(SUM((item->>'cantidad')::int), 0)::int AS reservado
           FROM catalog.reservas_ordenes ro,
                jsonb_array_elements(ro.items_json) AS item
           WHERE LOWER(item->>'sku') = LOWER(p.sku)
             AND ro.creado_en > NOW() - INTERVAL '30 minutes'
         ) r
    WHERE LOWER(s.sku) = v_sku AND LOWER(p.sku) = v_sku;
  END LOOP;
  RETURN NULL;
END;
$$;

-- Triggers
DROP TRIGGER IF EXISTS productos_proyeccion_trigger ON catalog.productos;
CREATE TRIGGER productos_proyeccion_trigger
AFTER INSERT OR UPDATE ON catalog.productos
FOR EACH ROW EXECUTE FUNCTION catalog.fn_trigger_producto();

DROP TRIGGER IF EXISTS reservas_proyeccion_trigger ON catalog.reservas_ordenes;
CREATE TRIGGER reservas_proyeccion_trigger
AFTER INSERT OR UPDATE OR DELETE ON catalog.reservas_ordenes
FOR EACH ROW EXECUTE FUNCTION catalog.fn_trigger_reserva();

-- Bootstrap inicial
SELECT catalog.fn_bootstrap_proyecciones();
