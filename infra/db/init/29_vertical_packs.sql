-- Fase 5: packs versionados para crear tenants sin cambios de codigo.
-- La migracion es aditiva: no elimina verticales ni capacidades existentes.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.packs (
  codigo           text NOT NULL CHECK (codigo ~ '^pack_[a-z][a-z0-9_]{1,39}$'),
  version          integer NOT NULL CHECK (version > 0),
  vertical_codigo  text NOT NULL REFERENCES identity.verticales(codigo),
  nombre           text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  descripcion      text NOT NULL DEFAULT '' CHECK (char_length(descripcion) <= 500),
  config           jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  activo           boolean NOT NULL DEFAULT true,
  creado_en        timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en   timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (codigo, version)
);

CREATE TABLE IF NOT EXISTS identity.pack_capacidades (
  pack_codigo      text NOT NULL,
  pack_version     integer NOT NULL,
  capacidad_codigo text NOT NULL REFERENCES identity.capacidades(codigo),
  config           jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  activo           boolean NOT NULL DEFAULT true,
  PRIMARY KEY (pack_codigo, pack_version, capacidad_codigo),
  FOREIGN KEY (pack_codigo, pack_version)
    REFERENCES identity.packs(codigo, version) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_pack_capacidades_activo
  ON identity.pack_capacidades (pack_codigo, pack_version, activo);

ALTER TABLE identity.empresas
  ADD COLUMN IF NOT EXISTS pack_codigo text;
ALTER TABLE identity.empresas
  ADD COLUMN IF NOT EXISTS pack_version integer;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'empresas_pack_completo_check'
  ) THEN
    ALTER TABLE identity.empresas
      ADD CONSTRAINT empresas_pack_completo_check
      CHECK ((pack_codigo IS NULL AND pack_version IS NULL)
          OR (pack_codigo IS NOT NULL AND pack_version IS NOT NULL));
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'empresas_pack_fk'
  ) THEN
    ALTER TABLE identity.empresas
      ADD CONSTRAINT empresas_pack_fk
      FOREIGN KEY (pack_codigo, pack_version)
      REFERENCES identity.packs(codigo, version);
  END IF;
END $$;
CREATE INDEX IF NOT EXISTS idx_empresas_pack
  ON identity.empresas (pack_codigo, pack_version);

INSERT INTO identity.packs (codigo, version, vertical_codigo, nombre, descripcion, config)
VALUES
  ('pack_retail', 1, 'retail', 'Retail', 'Venta de productos con inventario y canales de entrega.', '{"modelo":"comercio"}'::jsonb),
  ('pack_farmacia', 1, 'farmacia', 'Farmacia', 'Venta trazable de productos con lotes y vencimiento.', '{"modelo":"comercio_trazable"}'::jsonb),
  ('pack_belleza', 1, 'belleza', 'Belleza', 'Servicios reservables con profesionales y recursos.', '{"modelo":"servicios_agendados"}'::jsonb),
  ('pack_dental', 1, 'dental', 'Dental', 'Servicios dentales agendados sin historia clinica en esta fase.', '{"modelo":"servicios_agendados"}'::jsonb),
  ('pack_flota', 1, 'flota', 'Flota', 'Operacion logistica con vehiculos, rutas y cotizaciones.', '{"modelo":"logistica"}'::jsonb)
ON CONFLICT (codigo, version) DO UPDATE
SET vertical_codigo = EXCLUDED.vertical_codigo,
    nombre = EXCLUDED.nombre,
    descripcion = EXCLUDED.descripcion,
    config = EXCLUDED.config,
    actualizado_en = NOW();

INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT 'pack_' || vc.vertical_codigo, 1, vc.capacidad_codigo, vc.config, vc.activo
FROM identity.vertical_capacidades vc
WHERE vc.vertical_codigo IN ('retail', 'farmacia', 'belleza', 'dental', 'flota')
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO UPDATE
SET config = EXCLUDED.config,
    activo = EXCLUDED.activo;

COMMIT;
