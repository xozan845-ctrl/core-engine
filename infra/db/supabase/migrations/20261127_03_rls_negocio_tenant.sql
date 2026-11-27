-- RLS: tablas que solo tienen `negocio_id` (sin `empresa_id` ni `tenant_id`).
--
-- El onboarding crea la empresa y su negocio, pero el token todavía no lleva el
-- claim `negocio_id`. La política acepta el negocio cuando pertenece a la
-- empresa del claim `tenant_id`.
BEGIN;

DO $$
DECLARE r record;
BEGIN
  FOR r IN
    SELECT c.table_schema, c.table_name
      FROM information_schema.tables c
     WHERE c.table_schema = ANY (ARRAY[
       'identity','stores','orders','catalog','item','inventory','pricing',
       'commissions','finance','logistics','scheduling','field','notifications',
       'storage','analytics','intelligence'
     ])
       AND c.table_type = 'BASE TABLE'
       AND EXISTS (
             SELECT 1 FROM information_schema.columns col
              WHERE col.table_schema = c.table_schema
                AND col.table_name = c.table_name
                AND col.column_name = 'negocio_id'
           )
       AND NOT EXISTS (
             SELECT 1 FROM information_schema.columns col
              WHERE col.table_schema = c.table_schema
                AND col.table_name = c.table_name
                AND col.column_name IN ('empresa_id', 'tenant_id')
           )
  LOOP
    EXECUTE format(
      'DROP POLICY IF EXISTS rls_hardening_tenant ON %I.%I',
      r.table_schema, r.table_name
    );
    EXECUTE format($f$
      CREATE POLICY rls_hardening_tenant ON %I.%I FOR ALL TO authenticated
      USING (auth.rol() IN ('super_admin', 'admin')
             OR negocio_id::text = (auth.jwt() ->> 'negocio_id')
             OR EXISTS (
                  SELECT 1 FROM identity.negocios n
                   WHERE n.id = negocio_id
                     AND n.empresa_id::text = (auth.jwt() ->> 'tenant_id')
                ))
      WITH CHECK (auth.rol() IN ('super_admin', 'admin')
             OR negocio_id::text = (auth.jwt() ->> 'negocio_id')
             OR EXISTS (
                  SELECT 1 FROM identity.negocios n
                   WHERE n.id = negocio_id
                     AND n.empresa_id::text = (auth.jwt() ->> 'tenant_id')
                ))
    $f$, r.table_schema, r.table_name);
  END LOOP;
END $$;

COMMIT;
