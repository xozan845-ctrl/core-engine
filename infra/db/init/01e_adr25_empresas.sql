-- ADR-25: dominio empresarial, participacion de vendedores y logistica hibrida.
-- Migracion aditiva para bases existentes. Las migraciones 01/01c no se editan
-- (R-DB-6): este archivo aplica los cambios de esquema y redefine las funciones
-- de proyeccion del catalogo para incluir empresa_id.
BEGIN;

-- ── identity ─────────────────────────────────────────────────────────────
ALTER TABLE identity.usuarios DROP CONSTRAINT IF EXISTS usuarios_rol_check;
ALTER TABLE identity.usuarios ADD CONSTRAINT usuarios_rol_check
  CHECK (rol IN ('super_admin', 'admin', 'empresario', 'vendedor', 'comprador',
                 'logistica', 'coordinador', 'supervisor', 'operativo'));

ALTER TABLE identity.usuarios ADD COLUMN IF NOT EXISTS participacion_porcentaje integer;
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'usuarios_participacion_check'
  ) THEN
    ALTER TABLE identity.usuarios ADD CONSTRAINT usuarios_participacion_check
      CHECK (participacion_porcentaje BETWEEN 0 AND 100);
  END IF;
END $$;

CREATE TABLE IF NOT EXISTS identity.empresas (
  id                  uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  nombre              text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 160),
  modalidad_logistica text NOT NULL CHECK (modalidad_logistica IN ('plataforma', 'propia', 'hibrida')),
  estado              text NOT NULL DEFAULT 'activa' CHECK (estado IN ('activa', 'suspendida', 'inactiva')),
  creado_por          uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en           timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS identity.empresa_miembros (
  empresa_id                uuid NOT NULL REFERENCES identity.empresas(id),
  usuario_id                uuid NOT NULL REFERENCES identity.usuarios(id),
  rol                       text NOT NULL CHECK (rol IN ('empresario', 'vendedor')),
  participacion_porcentaje  integer CHECK (participacion_porcentaje BETWEEN 0 AND 100),
  estado                    text NOT NULL DEFAULT 'activo' CHECK (estado IN ('activo', 'suspendido', 'inactivo')),
  creado_en                 timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (empresa_id, usuario_id),
  CHECK ((rol = 'vendedor' AND participacion_porcentaje IS NOT NULL) OR
         (rol = 'empresario' AND participacion_porcentaje IS NULL))
);
CREATE INDEX IF NOT EXISTS idx_empresa_miembros_usuario
  ON identity.empresa_miembros (usuario_id, estado);

-- ── catalog ──────────────────────────────────────────────────────────────
-- Los productos legados pueden quedar sin empresa hasta que un super admin
-- los asigne; los productos nuevos siempre la informan.
ALTER TABLE catalog.productos ADD COLUMN IF NOT EXISTS empresa_id uuid;
ALTER TABLE catalog.producto_catalogo_vista ADD COLUMN IF NOT EXISTS empresa_id uuid;

-- ── stores ───────────────────────────────────────────────────────────────
ALTER TABLE stores.tiendas DROP CONSTRAINT IF EXISTS tiendas_vendedor_id_key;
ALTER TABLE stores.tiendas ADD COLUMN IF NOT EXISTS empresa_id uuid;
ALTER TABLE stores.tiendas ADD COLUMN IF NOT EXISTS propietario_usuario_id uuid;
ALTER TABLE stores.tiendas ADD COLUMN IF NOT EXISTS participacion_porcentaje integer;
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'tiendas_participacion_check'
  ) THEN
    ALTER TABLE stores.tiendas ADD CONSTRAINT tiendas_participacion_check
      CHECK (participacion_porcentaje BETWEEN 0 AND 100);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS idx_tiendas_empresa ON stores.tiendas (empresa_id);
CREATE INDEX IF NOT EXISTS idx_tiendas_propietario ON stores.tiendas (propietario_usuario_id);

-- ── logistics ────────────────────────────────────────────────────────────
ALTER TABLE logistics.envios DROP CONSTRAINT IF EXISTS envios_order_id_key;
ALTER TABLE logistics.envios ADD COLUMN IF NOT EXISTS empresa_id uuid;
ALTER TABLE logistics.envios ADD COLUMN IF NOT EXISTS modalidad_logistica text;
ALTER TABLE logistics.envios ADD COLUMN IF NOT EXISTS proveedor_tipo text;
ALTER TABLE logistics.envios ADD COLUMN IF NOT EXISTS proveedor_tenant_id uuid;
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'envios_modalidad_check'
  ) THEN
    ALTER TABLE logistics.envios ADD CONSTRAINT envios_modalidad_check
      CHECK (modalidad_logistica IN ('plataforma', 'propia', 'hibrida'));
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'envios_proveedor_tipo_check'
  ) THEN
    ALTER TABLE logistics.envios ADD CONSTRAINT envios_proveedor_tipo_check
      CHECK (proveedor_tipo IN ('plataforma', 'empresa'));
  END IF;
END $$;
CREATE UNIQUE INDEX IF NOT EXISTS idx_envios_orden_empresa
  ON logistics.envios (order_id, empresa_id);
CREATE INDEX IF NOT EXISTS idx_envios_empresa
  ON logistics.envios (empresa_id, creado_en);
CREATE INDEX IF NOT EXISTS idx_envios_proveedor
  ON logistics.envios (proveedor_tenant_id, creado_en);

CREATE TABLE IF NOT EXISTS logistics.asignaciones_envio (
  id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  envio_id uuid NOT NULL REFERENCES logistics.envios(id),
  proveedor_tipo_anterior text CHECK (proveedor_tipo_anterior IN ('plataforma', 'empresa')),
  proveedor_tipo_nuevo text NOT NULL CHECK (proveedor_tipo_nuevo IN ('plataforma', 'empresa')),
  proveedor_tenant_id uuid NOT NULL,
  origen text NOT NULL CHECK (origen IN ('automatica', 'empresario')),
  actor_usuario_id uuid,
  creado_en timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_asignaciones_envio
  ON logistics.asignaciones_envio (envio_id, creado_en);
CREATE UNIQUE INDEX IF NOT EXISTS idx_asignacion_automatica_unica
  ON logistics.asignaciones_envio (envio_id) WHERE origen = 'automatica';

-- ── commissions ──────────────────────────────────────────────────────────
ALTER TABLE commissions.comisiones DROP CONSTRAINT IF EXISTS comisiones_order_id_key;
ALTER TABLE commissions.comisiones ADD COLUMN IF NOT EXISTS linea_clave text;
ALTER TABLE commissions.comisiones ADD COLUMN IF NOT EXISTS empresa_id uuid;
ALTER TABLE commissions.comisiones ADD COLUMN IF NOT EXISTS tienda_id uuid;
ALTER TABLE commissions.comisiones ADD COLUMN IF NOT EXISTS monto_empresario_cents integer;
ALTER TABLE commissions.comisiones ADD COLUMN IF NOT EXISTS participacion_porcentaje integer;
DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'comisiones_participacion_check'
  ) THEN
    ALTER TABLE commissions.comisiones ADD CONSTRAINT comisiones_participacion_check
      CHECK (participacion_porcentaje BETWEEN 0 AND 100);
  END IF;
END $$;
CREATE UNIQUE INDEX IF NOT EXISTS idx_comisiones_orden_linea
  ON commissions.comisiones (order_id, linea_clave);

CREATE TABLE IF NOT EXISTS commissions.liquidaciones_empresas (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  periodo_inicio date NOT NULL,
  periodo_fin    date NOT NULL,
  monto_cents    integer NOT NULL,
  estado         text NOT NULL DEFAULT 'aprobada' CHECK (estado IN ('aprobada', 'pagada')),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, periodo_inicio)
);

ALTER TABLE commissions.compensaciones_devoluciones ADD COLUMN IF NOT EXISTS empresa_id uuid;
ALTER TABLE commissions.compensaciones_devoluciones ADD COLUMN IF NOT EXISTS monto_empresario_cents integer;

-- ── proyecciones del catalogo (redefinidas con empresa_id) ───────────────
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

COMMIT;
