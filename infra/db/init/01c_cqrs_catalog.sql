-- CQRS lógico (ADR-07): proyecciones de lectura para Catalog/Inventario. Idempotente.
-- Se ejecuta despues de 01_esquemas (orden alfabetico) tanto en fresh como en DB viva.
CREATE SCHEMA IF NOT EXISTS catalog;

-- 1) Vista de catálogo público (read-optimized para listados/búsqueda)
CREATE TABLE IF NOT EXISTS catalog.producto_catalogo_vista (
  id              uuid PRIMARY KEY,
  sku             text NOT NULL UNIQUE,
  nombre          text NOT NULL,
  descripcion     text,
  categoria       text NOT NULL,
  precio_base_cents integer NOT NULL,
  disponible      integer NOT NULL,          -- stock_fisico - reservado
  estado          text NOT NULL,             -- 'disponible' | 'agotado'
  creado_en       timestamptz NOT NULL,
  actualizado_en  timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_catalogo_categoria ON catalog.producto_catalogo_vista (categoria);
CREATE INDEX IF NOT EXISTS idx_catalogo_estado ON catalog.producto_catalogo_vista (estado);
-- pg_trgm no está garantizado; usar ILIKE o crear extensión si se necesita búsqueda full-text
-- CREATE INDEX IF NOT EXISTS idx_catalogo_nombre_trgm ON catalog.producto_catalogo_vista USING gin (nombre gin_trgm_ops);

-- 2) Vista de stock detallada (para admin, alertas, valorización)
CREATE TABLE IF NOT EXISTS catalog.stock_vista (
  sku                 text PRIMARY KEY,
  nombre              text NOT NULL,
  stock_fisico        integer NOT NULL DEFAULT 0,
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
  INSERT INTO catalog.producto_catalogo_vista (id, sku, nombre, descripcion, categoria, precio_base_cents, disponible, estado, creado_en, actualizado_en)
  SELECT p.id, p.sku, p.nombre, p.descripcion, p.categoria, p.precio_base_cents,
         GREATEST(p.stock - COALESCE(r.reservado, 0), 0),
         p.estado, p.creado_en, NOW()
  FROM catalog.productos p
  LEFT JOIN (
    SELECT LOWER(item->>'sku') AS sku, SUM((item->>'cantidad')::int) AS reservado
    FROM catalog.reservas_ordenes ro,
         jsonb_array_elements(ro.items_json) AS item
    WHERE ro.creado_en > NOW() - INTERVAL '30 minutes'
    GROUP BY LOWER(item->>'sku')
  ) r ON LOWER(p.sku) = r.sku;

  -- 2) stock_vista
  DELETE FROM catalog.stock_vista;
  INSERT INTO catalog.stock_vista (sku, nombre, stock_fisico, reservado, disponible, precio_base_cents, estado, actualizado_en)
  SELECT p.sku, p.nombre, p.stock,
         COALESCE(r.reservado, 0),
         GREATEST(p.stock - COALESCE(r.reservado, 0), 0),
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
      INSERT INTO catalog.producto_catalogo_vista (id, sku, nombre, descripcion, categoria, precio_base_cents, disponible, estado, creado_en, actualizado_en)
      VALUES (NEW.id, NEW.sku, NEW.nombre, NEW.descripcion, NEW.categoria, NEW.precio_base_cents,
              GREATEST(NEW.stock - v_reservado, 0), NEW.estado, NEW.creado_en, NOW());
    ELSE
      UPDATE catalog.producto_catalogo_vista
      SET nombre = NEW.nombre, descripcion = NEW.descripcion, categoria = NEW.categoria,
          precio_base_cents = NEW.precio_base_cents,
          disponible = GREATEST(NEW.stock - v_reservado, 0),
          estado = NEW.estado, actualizado_en = NOW()
      WHERE id = NEW.id;
    END IF;

    -- stock_vista
    IF TG_OP = 'INSERT' THEN
      INSERT INTO catalog.stock_vista (sku, nombre, stock_fisico, reservado, disponible, precio_base_cents, estado, actualizado_en)
      VALUES (NEW.sku, NEW.nombre, NEW.stock, v_reservado, GREATEST(NEW.stock - v_reservado, 0), NEW.precio_base_cents, NEW.estado, NOW());
    ELSE
      UPDATE catalog.stock_vista
      SET nombre = NEW.nombre, stock_fisico = NEW.stock, reservado = v_reservado,
          disponible = GREATEST(NEW.stock - v_reservado, 0),
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
  v_cant int;
  v_item jsonb;
BEGIN
  IF TG_OP IN ('INSERT', 'UPDATE') THEN
    FOR v_item IN SELECT jsonb_array_elements(NEW.items_json) LOOP
      v_sku := LOWER(v_item->>'sku');
      v_cant := (v_item->>'cantidad')::int;
      -- actualizar reservado en stock_vista y disponible en ambas vistas
      UPDATE catalog.stock_vista
      SET reservado = reservado + v_cant,
          disponible = GREATEST(stock_fisico - (reservado + v_cant), 0),
          actualizado_en = NOW()
      WHERE LOWER(sku) = v_sku;
      UPDATE catalog.producto_catalogo_vista
      SET disponible = GREATEST(disponible - v_cant, 0),
          actualizado_en = NOW()
      WHERE LOWER(sku) = v_sku;
    END LOOP;
  ELSIF TG_OP = 'DELETE' THEN
    FOR v_item IN SELECT jsonb_array_elements(OLD.items_json) LOOP
      v_sku := LOWER(v_item->>'sku');
      v_cant := (v_item->>'cantidad')::int;
      UPDATE catalog.stock_vista
      SET reservado = GREATEST(reservado - v_cant, 0),
          disponible = GREATEST(stock_fisico - GREATEST(reservado - v_cant, 0), 0),
          actualizado_en = NOW()
      WHERE LOWER(sku) = v_sku;
      UPDATE catalog.producto_catalogo_vista
      SET disponible = disponible + v_cant,
          actualizado_en = NOW()
      WHERE LOWER(sku) = v_sku;
    END LOOP;
  END IF;
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