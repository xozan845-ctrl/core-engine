-- Fase 1 de universalizacion: modelos operativos y constructor de packs.
-- La migracion es aditiva: conserva verticales, packs y tenants existentes.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.modelos_operacion (
  codigo       text PRIMARY KEY CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre       text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  descripcion  text NOT NULL DEFAULT '' CHECK (char_length(descripcion) <= 500),
  config       jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  activo       boolean NOT NULL DEFAULT true,
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS identity.pack_modelos (
  pack_codigo   text NOT NULL,
  pack_version  integer NOT NULL,
  modelo_codigo text NOT NULL REFERENCES identity.modelos_operacion(codigo),
  config        jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(config) = 'object'),
  activo        boolean NOT NULL DEFAULT true,
  PRIMARY KEY (pack_codigo, pack_version, modelo_codigo),
  FOREIGN KEY (pack_codigo, pack_version)
    REFERENCES identity.packs(codigo, version) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_pack_modelos_activo
  ON identity.pack_modelos (pack_codigo, pack_version, activo);

INSERT INTO identity.modelos_operacion (codigo, nombre, descripcion, config)
VALUES
  ('comercio', 'Comercio', 'Venta de productos con catálogo y precios.', '{"items":["producto"]}'::jsonb),
  ('comercio_trazable', 'Comercio trazable', 'Comercio con inventario, lotes o vencimiento.', '{"items":["producto"],"trazabilidad":true}'::jsonb),
  ('servicios_agendados', 'Servicios agendados', 'Servicios con duración, recursos y citas.', '{"items":["servicio"],"requiere_agenda":true}'::jsonb),
  ('logistica', 'Logística', 'Operaciones con vehículos, rutas o entregas.', '{"fulfillment":["delivery","pickup","onsite"]}'::jsonb),
  ('mixto', 'Operación mixta', 'Combinación de productos, servicios y logística.', '{"items":["producto","servicio","alquiler"]}'::jsonb)
ON CONFLICT (codigo) DO UPDATE
SET nombre = EXCLUDED.nombre,
    descripcion = EXCLUDED.descripcion,
    config = EXCLUDED.config,
    actualizado_en = NOW();

INSERT INTO identity.pack_modelos (pack_codigo, pack_version, modelo_codigo, config)
SELECT p.codigo, p.version, p.config ->> 'modelo', '{}'::jsonb
  FROM identity.packs p
  JOIN identity.modelos_operacion mo ON mo.codigo = p.config ->> 'modelo'
 WHERE jsonb_typeof(p.config) = 'object'
ON CONFLICT (pack_codigo, pack_version, modelo_codigo) DO NOTHING;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA identity TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.modelos_operacion TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.pack_modelos TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.modelos_operacion ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS modelos_operacion_lectura ON identity.modelos_operacion;
DROP POLICY IF EXISTS modelos_operacion_admin ON identity.modelos_operacion;
CREATE POLICY modelos_operacion_lectura ON identity.modelos_operacion
  FOR SELECT TO anon, authenticated USING (activo);
CREATE POLICY modelos_operacion_admin ON identity.modelos_operacion
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.pack_modelos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS pack_modelos_lectura ON identity.pack_modelos;
DROP POLICY IF EXISTS pack_modelos_admin ON identity.pack_modelos;
CREATE POLICY pack_modelos_lectura ON identity.pack_modelos
  FOR SELECT TO authenticated
  USING (EXISTS (
    SELECT 1 FROM identity.packs p
     WHERE p.codigo = pack_codigo AND p.version = pack_version AND p.activo
  ));
CREATE POLICY pack_modelos_admin ON identity.pack_modelos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

COMMIT;
