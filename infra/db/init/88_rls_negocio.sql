-- ADR-0021 iteración 8: RLS por negocio (defensa en profundidad).
--
-- Las policies permisivas existentes siguen aislando por empresa. Se añade una
-- policy RESTRICTIVE por tabla con `negocio_id` para que un JWT con negocio
-- activo no pueda leer filas de otros negocios de la misma empresa.
-- Las filas con `negocio_id NULL` siguen siendo transversales.
BEGIN;

CREATE OR REPLACE FUNCTION auth.negocio_actual() RETURNS text
LANGUAGE sql STABLE AS $$
  SELECT NULLIF(auth.jwt() ->> 'negocio_id', '')
$$;

DO $$
DECLARE
  tabla text;
  nombre text;
BEGIN
  FOREACH tabla IN ARRAY ARRAY[
    'identity.roles_empresa',
    'identity.puestos_empresa',
    'identity.asignaciones_personal',
    'identity.invitaciones_personal',
    'identity.negocio_capacidades',
    'stores.tiendas',
    'item.items',
    'orders.orden_vista',
    'commissions.comisiones',
    'commissions.liquidaciones_empresas',
    'commissions.compensaciones_devoluciones',
    'finance.comprobantes',
    'logistics.envios'
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

-- Ofertas no guardan negocio directo: se resuelve por su tienda.
DROP POLICY IF EXISTS stores_ofertas_negocio_restrictiva ON stores.ofertas;
CREATE POLICY stores_ofertas_negocio_restrictiva ON stores.ofertas
  AS RESTRICTIVE FOR ALL TO authenticated
  USING (auth.negocio_actual() IS NULL OR EXISTS (
    SELECT 1 FROM stores.tiendas t
     WHERE t.id = tienda_id
       AND (t.negocio_id IS NULL OR t.negocio_id::text = auth.negocio_actual())
  ))
  WITH CHECK (auth.negocio_actual() IS NULL OR EXISTS (
    SELECT 1 FROM stores.tiendas t
     WHERE t.id = tienda_id
       AND (t.negocio_id IS NULL OR t.negocio_id::text = auth.negocio_actual())
  ));

COMMIT;
