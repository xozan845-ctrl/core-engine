-- ADR-0019 iteración 9: inventario completo por negocio.
--
-- Ubicaciones, proveedores y configuración pueden acotarse a un negocio.
-- `negocio_id NULL` conserva el comportamiento compartido por empresa.
BEGIN;

ALTER TABLE inventory.ubicaciones ADD COLUMN IF NOT EXISTS negocio_id uuid;
ALTER TABLE inventory.proveedores ADD COLUMN IF NOT EXISTS negocio_id uuid;
ALTER TABLE inventory.configuracion_inventario ADD COLUMN IF NOT EXISTS negocio_id uuid;

UPDATE inventory.ubicaciones u
   SET negocio_id = n.id
  FROM identity.negocios n
 WHERE n.empresa_id = u.empresa_id AND n.es_predeterminado AND u.negocio_id IS NULL;

UPDATE inventory.configuracion_inventario c
   SET negocio_id = n.id
  FROM identity.negocios n
 WHERE n.empresa_id = c.empresa_id AND n.es_predeterminado AND c.negocio_id IS NULL;

-- La configuración pasa de una fila por empresa a una por (empresa, negocio).
ALTER TABLE inventory.configuracion_inventario
  DROP CONSTRAINT IF EXISTS configuracion_inventario_pkey;
CREATE UNIQUE INDEX IF NOT EXISTS idx_configuracion_inventario_empresa_negocio
  ON inventory.configuracion_inventario (empresa_id,
    COALESCE(negocio_id, '00000000-0000-0000-0000-000000000000'::uuid));

CREATE INDEX IF NOT EXISTS idx_ubicaciones_negocio
  ON inventory.ubicaciones (empresa_id, negocio_id, activo);
CREATE INDEX IF NOT EXISTS idx_proveedores_negocio
  ON inventory.proveedores (empresa_id, negocio_id, activo);

-- Defensa en profundidad: las policies permisivas siguen por empresa.
DO $$
DECLARE
  tabla text;
  nombre text;
BEGIN
  FOREACH tabla IN ARRAY ARRAY[
    'inventory.ubicaciones',
    'inventory.proveedores',
    'inventory.configuracion_inventario'
  ] LOOP
    nombre := replace(tabla, '.', '_') || '_negocio_restrictiva';
    EXECUTE format('DROP POLICY IF EXISTS %I ON %s', nombre, tabla);
    EXECUTE format(
      'CREATE POLICY %I ON %s AS RESTRICTIVE FOR ALL TO authenticated
         USING (auth.negocio_actual() IS NULL
                OR negocio_id IS NULL
                OR negocio_id::text = auth.negocio_actual())
         WITH CHECK (auth.negocio_actual() IS NULL
                OR negocio_id IS NULL
                OR negocio_id::text = auth.negocio_actual())',
      nombre, tabla
    );
  END LOOP;
END $$;

COMMIT;
