-- ADR-0021 iteración 12: administración de negocios.
BEGIN;

ALTER TABLE identity.negocios
  ADD COLUMN IF NOT EXISTS archivado_en timestamptz;

CREATE TABLE IF NOT EXISTS identity.plantillas_roles (
  vertical_id text NOT NULL REFERENCES identity.verticales(codigo),
  codigo      text NOT NULL CHECK (codigo ~ '^[a-z][a-z0-9_]{1,39}$'),
  nombre      text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  permisos    jsonb NOT NULL CHECK (jsonb_typeof(permisos) = 'object'),
  activo      boolean NOT NULL DEFAULT true,
  PRIMARY KEY (vertical_id, codigo)
);

-- Una ubicación predeterminada por negocio.
DROP INDEX IF EXISTS inventory.ux_ubicaciones_predeterminada;
CREATE UNIQUE INDEX IF NOT EXISTS ux_ubicaciones_predeterminada_negocio
  ON inventory.ubicaciones (empresa_id,
    COALESCE(negocio_id, '00000000-0000-0000-0000-000000000000'::uuid))
  WHERE predeterminada AND activo;

INSERT INTO identity.plantillas_roles (vertical_id, codigo, nombre, permisos) VALUES
  ('retail', 'cajero', 'Cajero', '{"acciones":["catalogo.leer","ordenes.leer"]}'::jsonb),
  ('retail', 'supervisor', 'Supervisor', '{"acciones":["catalogo.leer","catalogo.escribir","inventario.leer","inventario.escribir","ordenes.leer","ordenes.gestionar"]}'::jsonb),
  ('farmacia', 'regente', 'Regente', '{"acciones":["catalogo.leer","catalogo.escribir","inventario.leer","inventario.escribir","ordenes.leer"]}'::jsonb),
  ('farmacia', 'cajero', 'Cajero', '{"acciones":["catalogo.leer","ordenes.leer"]}'::jsonb),
  ('belleza', 'recepcionista', 'Recepcionista', '{"acciones":["agenda.leer","agenda.escribir","ordenes.leer","ordenes.crear"]}'::jsonb),
  ('belleza', 'profesional', 'Profesional', '{"acciones":["agenda.leer","catalogo.leer"]}'::jsonb),
  ('dental', 'recepcionista', 'Recepcionista', '{"acciones":["agenda.leer","agenda.escribir","ordenes.leer","ordenes.crear"]}'::jsonb),
  ('dental', 'profesional', 'Profesional', '{"acciones":["agenda.leer","catalogo.leer"]}'::jsonb),
  ('flota', 'despachador', 'Despachador', '{"acciones":["logistica.envios.leer","logistica.envios.asignar","logistica.envios.actualizar","logistica.ordenes.actualizar","ordenes.leer"]}'::jsonb)
ON CONFLICT (vertical_id, codigo) DO UPDATE
  SET nombre = EXCLUDED.nombre, permisos = EXCLUDED.permisos, activo = true;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.plantillas_roles TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.plantillas_roles ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS plantillas_roles_admin ON identity.plantillas_roles;
DROP POLICY IF EXISTS plantillas_roles_tenant ON identity.plantillas_roles;
DROP POLICY IF EXISTS plantillas_roles_app ON identity.plantillas_roles;
CREATE POLICY plantillas_roles_admin ON identity.plantillas_roles FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY plantillas_roles_tenant ON identity.plantillas_roles FOR SELECT TO authenticated
  USING (true);
CREATE POLICY plantillas_roles_app ON identity.plantillas_roles FOR SELECT TO core_engine_app
  USING (true);

COMMIT;
