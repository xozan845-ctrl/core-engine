-- ADR-0001: tenancy multi-vertical y capacidades activables.
-- Migracion aditiva: conserva empresas, usuarios y contratos existentes.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.verticales (
  codigo              text PRIMARY KEY CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre              text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  modelo_operacion    text NOT NULL CHECK (char_length(trim(modelo_operacion)) BETWEEN 2 AND 160),
  activo              boolean NOT NULL DEFAULT true,
  creado_en           timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS identity.capacidades (
  codigo              text PRIMARY KEY CHECK (codigo ~ '^[a-z][a-z0-9_]{1,59}$'),
  nombre              text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  modulo              text NOT NULL CHECK (char_length(trim(modulo)) BETWEEN 2 AND 80),
  activo              boolean NOT NULL DEFAULT true,
  creado_en           timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS identity.vertical_capacidades (
  vertical_codigo     text NOT NULL REFERENCES identity.verticales(codigo),
  capacidad_codigo    text NOT NULL REFERENCES identity.capacidades(codigo),
  config              jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  activo              boolean NOT NULL DEFAULT true,
  PRIMARY KEY (vertical_codigo, capacidad_codigo)
);

CREATE TABLE IF NOT EXISTS identity.empresa_capacidades (
  empresa_id          uuid NOT NULL REFERENCES identity.empresas(id),
  capacidad_codigo    text NOT NULL REFERENCES identity.capacidades(codigo),
  config              jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  activo              boolean NOT NULL DEFAULT true,
  creado_en           timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en      timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (empresa_id, capacidad_codigo)
);
CREATE INDEX IF NOT EXISTS idx_empresa_capacidades_activo
  ON identity.empresa_capacidades (empresa_id, activo);

ALTER TABLE identity.empresas
  ADD COLUMN IF NOT EXISTS vertical_id text;
ALTER TABLE identity.empresas
  ALTER COLUMN modalidad_logistica DROP NOT NULL;
COMMENT ON COLUMN identity.empresas.modalidad_logistica IS
  'Legado nullable; la operacion logistica se configura por capacidades y fulfillment_config.';

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'empresas_vertical_id_fkey'
  ) THEN
    ALTER TABLE identity.empresas
      ADD CONSTRAINT empresas_vertical_id_fkey
      FOREIGN KEY (vertical_id) REFERENCES identity.verticales(codigo);
  END IF;
END $$;

INSERT INTO identity.verticales (codigo, nombre, modelo_operacion) VALUES
  ('retail', 'Retail', 'Venta de productos con inventario y canales de entrega'),
  ('farmacia', 'Farmacia', 'Venta de productos con lotes, vencimiento y trazabilidad'),
  ('belleza', 'Belleza', 'Venta de servicios con agenda y recursos reservables'),
  ('dental', 'Dental', 'Venta de servicios con agenda; capacidades clinicas posteriores'),
  ('flota', 'Flota', 'Operacion logistica de vehiculos, rutas y servicios cotizables')
ON CONFLICT (codigo) DO NOTHING;

INSERT INTO identity.capacidades (codigo, nombre, modulo) VALUES
  ('catalogo_items', 'Catalogo de items', 'catalog'),
  ('inventario_stock', 'Inventario y stock', 'catalog'),
  ('inventario_lotes', 'Lotes y vencimiento', 'catalog'),
  ('codigo_barras', 'Codigo de barras', 'catalog'),
  ('agenda_recursos', 'Agenda y recursos', 'scheduling'),
  ('historia_clinica', 'Historia clinica', 'clinical'),
  ('recetas', 'Recetas', 'clinical'),
  ('fulfillment_delivery', 'Delivery', 'logistics'),
  ('fulfillment_pickup', 'Pickup', 'logistics'),
  ('fulfillment_onsite', 'Onsite', 'logistics'),
  ('logistica_rutas', 'Rutas y vehiculos', 'logistics'),
  ('cotizaciones', 'Cotizaciones', 'orders'),
  ('facturacion', 'Facturacion', 'finance'),
  ('liquidaciones', 'Liquidaciones', 'commissions')
ON CONFLICT (codigo) DO NOTHING;

-- Defaults versionables por vertical. Las capacidades opcionales se habilitan
-- por empresa en el alta o mediante la API de identity.
INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo) VALUES
  ('retail', 'catalogo_items'),
  ('retail', 'inventario_stock'),
  ('retail', 'codigo_barras'),
  ('retail', 'fulfillment_delivery'),
  ('retail', 'fulfillment_pickup'),
  ('retail', 'facturacion'),
  ('retail', 'liquidaciones'),
  ('farmacia', 'catalogo_items'),
  ('farmacia', 'inventario_stock'),
  ('farmacia', 'inventario_lotes'),
  ('farmacia', 'codigo_barras'),
  ('farmacia', 'facturacion'),
  ('farmacia', 'liquidaciones'),
  ('belleza', 'catalogo_items'),
  ('belleza', 'agenda_recursos'),
  ('belleza', 'facturacion'),
  ('belleza', 'liquidaciones'),
  ('dental', 'catalogo_items'),
  ('dental', 'agenda_recursos'),
  ('dental', 'cotizaciones'),
  ('dental', 'facturacion'),
  ('dental', 'liquidaciones'),
  ('flota', 'catalogo_items'),
  ('flota', 'fulfillment_delivery'),
  ('flota', 'fulfillment_pickup'),
  ('flota', 'fulfillment_onsite'),
  ('flota', 'logistica_rutas'),
  ('flota', 'cotizaciones'),
  ('flota', 'facturacion'),
  ('flota', 'liquidaciones')
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo) VALUES
  ('retail', 'fulfillment_onsite', '{}'::jsonb, false),
  ('retail', 'cotizaciones', '{}'::jsonb, false),
  ('farmacia', 'fulfillment_delivery', '{}'::jsonb, false),
  ('farmacia', 'fulfillment_pickup', '{}'::jsonb, false),
  ('belleza', 'fulfillment_delivery', '{}'::jsonb, false),
  ('belleza', 'fulfillment_pickup', '{}'::jsonb, false),
  ('belleza', 'cotizaciones', '{}'::jsonb, false),
  ('flota', 'agenda_recursos', '{}'::jsonb, false)
ON CONFLICT (vertical_codigo, capacidad_codigo) DO UPDATE
  SET config = EXCLUDED.config, activo = EXCLUDED.activo;

-- Backfill seguro: las empresas comerciales existentes se comportan como
-- retail; las de tipo logistica como flota. No se toca el comportamiento
-- legado ni se borran datos.
UPDATE identity.empresas
SET vertical_id = CASE
  WHEN tipo_empresa = 'logistica' THEN 'flota'
  ELSE 'retail'
END
WHERE vertical_id IS NULL;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, vc.capacidad_codigo, vc.config, vc.activo
FROM identity.empresas e
JOIN identity.vertical_capacidades vc ON vc.vertical_codigo = e.vertical_id
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

COMMIT;
