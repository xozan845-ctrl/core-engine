-- RLS: los triggers de historial de slugs son SECURITY DEFINER y escriben como
-- owner; con FORCE RLS necesitan una política explícita. El rol de aplicación
-- sigue aislado por tenant: el owner es código de confianza.
BEGIN;

DO $$
BEGIN
  EXECUTE 'DROP POLICY IF EXISTS empresa_slugs_owner ON identity.empresa_slugs';
  EXECUTE format(
    'CREATE POLICY empresa_slugs_owner ON identity.empresa_slugs FOR ALL TO %I USING (true) WITH CHECK (true)',
    current_user
  );
  EXECUTE 'DROP POLICY IF EXISTS tienda_slugs_owner ON stores.tienda_slugs';
  EXECUTE format(
    'CREATE POLICY tienda_slugs_owner ON stores.tienda_slugs FOR ALL TO %I USING (true) WITH CHECK (true)',
    current_user
  );
END $$;

COMMIT;
