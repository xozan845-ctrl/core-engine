-- RLS del esquema field (R-SB-2, piloto flota).
-- Cada tabla de negocio se aísla por tenant_id; las tablas internas de eventos
-- y outbox quedan solo para roles administrativos, igual que scheduling.
BEGIN;

CREATE SCHEMA IF NOT EXISTS auth;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
     WHERE n.nspname = 'auth' AND p.proname = 'jwt' AND p.pronargs = 0
  ) THEN
    CREATE FUNCTION auth.jwt() RETURNS jsonb
      LANGUAGE sql STABLE
      AS $fn$ SELECT COALESCE(NULLIF(current_setting('request.jwt.claims', true), ''), '{}')::jsonb $fn$;
  END IF;
  IF NOT EXISTS (
    SELECT 1 FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
     WHERE n.nspname = 'auth' AND p.proname = 'rol' AND p.pronargs = 0
  ) THEN
    CREATE FUNCTION auth.rol() RETURNS text
      LANGUAGE sql STABLE
      AS $fn$ SELECT COALESCE(auth.jwt() ->> 'rol', 'anon') $fn$;
  END IF;
END $$;

DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN CREATE ROLE anon NOLOGIN; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN CREATE ROLE authenticated NOLOGIN; END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN CREATE ROLE core_engine_app NOLOGIN; END IF;
  GRANT authenticated TO core_engine_app;
END $$;

GRANT USAGE ON SCHEMA auth TO anon, authenticated;
GRANT EXECUTE ON FUNCTION auth.jwt() TO anon, authenticated;
GRANT EXECUTE ON FUNCTION auth.rol() TO anon, authenticated;
GRANT USAGE ON SCHEMA field TO core_engine_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA field TO core_engine_app;

DO $$
DECLARE
  tabla text;
BEGIN
  FOREACH tabla IN ARRAY ARRAY[
    'personal', 'clientes', 'vehiculos', 'rutas', 'paradas', 'pedidos',
    'asistencia', 'incidencias', 'tracking', 'visitas', 'cumplimiento'
  ] LOOP
    EXECUTE format('ALTER TABLE field.%I ENABLE ROW LEVEL SECURITY', tabla);
    EXECUTE format('DROP POLICY IF EXISTS field_%s_admin ON field.%I', tabla, tabla);
    EXECUTE format(
      'CREATE POLICY field_%s_admin ON field.%I FOR ALL TO authenticated
         USING (auth.rol() IN (''super_admin'', ''admin''))
         WITH CHECK (auth.rol() IN (''super_admin'', ''admin''))',
      tabla, tabla
    );
    EXECUTE format('DROP POLICY IF EXISTS field_%s_tenant ON field.%I', tabla, tabla);
    EXECUTE format(
      'CREATE POLICY field_%s_tenant ON field.%I FOR ALL TO authenticated
         USING (auth.rol() IN (''empresario'', ''logistica'', ''coordinador'', ''supervisor'', ''operativo'')
                AND tenant_id::text = (auth.jwt() ->> ''tenant_id''))
         WITH CHECK (auth.rol() IN (''empresario'', ''logistica'', ''coordinador'', ''supervisor'', ''operativo'')
                AND tenant_id::text = (auth.jwt() ->> ''tenant_id''))',
      tabla, tabla
    );
  END LOOP;
END $$;

ALTER TABLE field.eventos_procesados ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_eventos_procesados_admin ON field.eventos_procesados;
CREATE POLICY field_eventos_procesados_admin ON field.eventos_procesados
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE field.outbox ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_outbox_admin ON field.outbox;
CREATE POLICY field_outbox_admin ON field.outbox
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

COMMIT;
