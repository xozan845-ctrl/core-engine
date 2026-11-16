-- ADR-0021 iteración 7: catálogo de permisos por acción.
--
-- Las acciones se versionan por módulo y dejan de ser texto libre en los
-- roles. Los servicios validan que el token incluya la acción requerida.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.permisos (
  codigo       text PRIMARY KEY CHECK (codigo ~ '^[a-z][a-z0-9_.]{1,79}$'),
  modulo       text NOT NULL CHECK (char_length(trim(modulo)) BETWEEN 2 AND 80),
  nombre       text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  descripcion  text NOT NULL DEFAULT '' CHECK (char_length(descripcion) <= 500),
  version      integer NOT NULL DEFAULT 1 CHECK (version > 0),
  activo       boolean NOT NULL DEFAULT true,
  creado_en    timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_permisos_modulo
  ON identity.permisos (modulo, activo, codigo);

INSERT INTO identity.permisos (codigo, modulo, nombre, descripcion) VALUES
  ('agenda.leer', 'scheduling', 'Leer agenda', 'Consultar citas, recursos y disponibilidad.'),
  ('agenda.escribir', 'scheduling', 'Gestionar agenda', 'Crear y modificar citas y recursos.'),
  ('capacidades.leer', 'identity', 'Leer capacidades', 'Consultar capacidades de empresa o negocio.'),
  ('capacidades.gestionar', 'identity', 'Gestionar capacidades', 'Activar o desactivar capacidades.'),
  ('catalogo.leer', 'catalog', 'Leer catálogo', 'Consultar items, precios e inventario.'),
  ('catalogo.escribir', 'catalog', 'Gestionar catálogo', 'Crear y modificar items, precios e inventario.'),
  ('finanzas.leer', 'finance', 'Leer finanzas', 'Consultar libros, comprobantes y reportes.'),
  ('inventario.leer', 'inventory', 'Leer inventario', 'Consultar stock, lotes y movimientos.'),
  ('inventario.escribir', 'inventory', 'Gestionar inventario', 'Registrar movimientos, lotes y ajustes.'),
  ('logistica.envios.leer', 'logistics', 'Leer envíos', 'Consultar guías y estado de envíos.'),
  ('logistica.envios.asignar', 'logistics', 'Asignar envíos', 'Asignar proveedor o flota a un envío.'),
  ('logistica.envios.actualizar', 'logistics', 'Actualizar envíos', 'Avanzar el estado de un envío.'),
  ('logistica.ordenes.actualizar', 'logistics', 'Actualizar orden por logística', 'Avanzar el estado de una orden desde logística.'),
  ('ordenes.leer', 'orders', 'Leer órdenes', 'Consultar órdenes, timeline y reportes.'),
  ('ordenes.crear', 'orders', 'Crear órdenes', 'Crear órdenes y cotizaciones.'),
  ('ordenes.gestionar', 'orders', 'Gestionar órdenes', 'Cambiar estados y resolver cotizaciones.'),
  ('personal.leer', 'identity', 'Leer personal', 'Consultar roles, puestos, asignaciones e invitaciones.'),
  ('personal.gestionar', 'identity', 'Gestionar personal', 'Crear roles, puestos, asignaciones e invitaciones.'),
  ('tiendas.leer', 'stores', 'Leer tiendas', 'Consultar tiendas y ofertas.'),
  ('tiendas.gestionar', 'stores', 'Gestionar tiendas', 'Crear y modificar tiendas.'),
  ('tiendas.ofertas.leer', 'stores', 'Leer ofertas', 'Consultar ofertas de una tienda.'),
  ('tiendas.ofertas.gestionar', 'stores', 'Gestionar ofertas', 'Publicar y modificar ofertas.')
ON CONFLICT (codigo) DO UPDATE
  SET modulo = EXCLUDED.modulo,
      nombre = EXCLUDED.nombre,
      descripcion = EXCLUDED.descripcion,
      version = identity.permisos.version + 1,
      activo = true;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT ON identity.permisos TO core_engine_app;
  END IF;
END $$;

ALTER TABLE identity.permisos ENABLE ROW LEVEL SECURITY;

DROP POLICY IF EXISTS permisos_admin ON identity.permisos;
DROP POLICY IF EXISTS permisos_lectura ON identity.permisos;
DROP POLICY IF EXISTS permisos_app ON identity.permisos;
CREATE POLICY permisos_admin ON identity.permisos FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY permisos_lectura ON identity.permisos FOR SELECT TO authenticated
  USING (activo = true);
CREATE POLICY permisos_app ON identity.permisos FOR SELECT TO core_engine_app
  USING (true);

COMMIT;
