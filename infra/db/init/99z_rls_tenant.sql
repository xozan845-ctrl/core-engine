-- RLS efectivo para tablas de tenant.
--
-- 1) Elimina las políticas `TO core_engine_app USING (true)` de tablas con
--    datos de tenant (dejan pasar todo).
-- 2) Habilita RLS y crea una política `tenant_app` por empresa/tenant/negocio.
-- 3) Excluye auth, outbox, eventos procesados, jobs y catálogos de plataforma,
--    que tienen su propio contrato.
--
-- Idempotente: se puede aplicar dos veces.
BEGIN;

DO $$
DECLARE
  r record;
  excluidos text[] := ARRAY[
    'identity.usuarios',
    'identity.sesiones',
    'identity.password_reset_tokens',
    'identity.correo_verificacion_tokens',
    'identity.mfa_codigos_recuperacion',
    'identity.permisos',
    'identity.plantillas_roles',
    'identity.capacidades',
    'identity.verticales',
    'identity.vertical_capacidades',
    'identity.packs',
    'identity.pack_capacidades',
    'identity.modelos_operacion',
    'orders.eventos',
    'finance.asientos',
    'finance.asiento_detalles',
    'finance.plan_cuentas',
    'finance.libro_mayor_mv',
    'identity.auditoria'
  ];
BEGIN
  -- Políticas de bypass del rol de app (se conservan outbox/jobs/catálogos).
  FOR r IN
    SELECT p.schemaname, p.tablename, p.policyname
      FROM pg_policies p
     WHERE 'core_engine_app' = ANY (p.roles)
       AND p.qual = 'true'
       AND (p.schemaname || '.' || p.tablename) <> ALL (excluidos)
       AND p.tablename NOT LIKE '%outbox'
       AND p.tablename NOT LIKE '%eventos_procesados'
  LOOP
    EXECUTE format('DROP POLICY IF EXISTS %I ON %I.%I', r.policyname, r.schemaname, r.tablename);
  END LOOP;

  -- Tablas con empresa_id.
  FOR r IN
    SELECT c.table_schema, c.table_name
      FROM information_schema.columns c
      JOIN information_schema.tables t
        ON t.table_schema = c.table_schema AND t.table_name = c.table_name
     WHERE c.column_name = 'empresa_id'
       AND t.table_type = 'BASE TABLE'
       AND (c.table_schema || '.' || c.table_name) <> ALL (excluidos)
       AND c.table_name NOT LIKE '%outbox'
       AND c.table_name NOT LIKE '%eventos_procesados'
  LOOP
    EXECUTE format('ALTER TABLE %I.%I ENABLE ROW LEVEL SECURITY', r.table_schema, r.table_name);
    EXECUTE format('DROP POLICY IF EXISTS tenant_app ON %I.%I', r.table_schema, r.table_name);
    EXECUTE format($f$
      CREATE POLICY tenant_app ON %I.%I FOR ALL TO authenticated
      USING (auth.rol() IN ('super_admin', 'admin')
             OR empresa_id::text = (auth.jwt() ->> 'tenant_id'))
      WITH CHECK (auth.rol() IN ('super_admin', 'admin')
             OR empresa_id::text = (auth.jwt() ->> 'tenant_id'))
    $f$, r.table_schema, r.table_name);
  END LOOP;

  -- Tablas con tenant_id y sin empresa_id (storage, etc.).
  FOR r IN
    SELECT c.table_schema, c.table_name
      FROM information_schema.columns c
      JOIN information_schema.tables t
        ON t.table_schema = c.table_schema AND t.table_name = c.table_name
     WHERE c.column_name = 'tenant_id'
       AND t.table_type = 'BASE TABLE'
       AND (c.table_schema || '.' || c.table_name) <> ALL (excluidos)
       AND c.table_name NOT LIKE '%outbox'
       AND c.table_name NOT LIKE '%eventos_procesados'
       AND NOT EXISTS (
         SELECT 1 FROM information_schema.columns e
          WHERE e.table_schema = c.table_schema
            AND e.table_name = c.table_name
            AND e.column_name = 'empresa_id')
  LOOP
    EXECUTE format('ALTER TABLE %I.%I ENABLE ROW LEVEL SECURITY', r.table_schema, r.table_name);
    EXECUTE format('DROP POLICY IF EXISTS tenant_app ON %I.%I', r.table_schema, r.table_name);
    EXECUTE format($f$
      CREATE POLICY tenant_app ON %I.%I FOR ALL TO authenticated
      USING (auth.rol() IN ('super_admin', 'admin')
             OR tenant_id::text = (auth.jwt() ->> 'tenant_id'))
      WITH CHECK (auth.rol() IN ('super_admin', 'admin')
             OR tenant_id::text = (auth.jwt() ->> 'tenant_id'))
    $f$, r.table_schema, r.table_name);
  END LOOP;

  -- Tablas solo con negocio_id.
  FOR r IN
    SELECT c.table_schema, c.table_name
      FROM information_schema.columns c
      JOIN information_schema.tables t
        ON t.table_schema = c.table_schema AND t.table_name = c.table_name
     WHERE c.column_name = 'negocio_id'
       AND t.table_type = 'BASE TABLE'
       AND (c.table_schema || '.' || c.table_name) <> ALL (excluidos)
       AND c.table_name NOT LIKE '%outbox'
       AND c.table_name NOT LIKE '%eventos_procesados'
       AND NOT EXISTS (
         SELECT 1 FROM information_schema.columns e
          WHERE e.table_schema = c.table_schema
            AND e.table_name = c.table_name
            AND e.column_name IN ('empresa_id', 'tenant_id'))
  LOOP
    EXECUTE format('ALTER TABLE %I.%I ENABLE ROW LEVEL SECURITY', r.table_schema, r.table_name);
    EXECUTE format('DROP POLICY IF EXISTS tenant_app ON %I.%I', r.table_schema, r.table_name);
    EXECUTE format($f$
      CREATE POLICY tenant_app ON %I.%I FOR ALL TO authenticated
      USING (auth.rol() IN ('super_admin', 'admin')
             OR negocio_id::text = (auth.jwt() ->> 'negocio_id'))
      WITH CHECK (auth.rol() IN ('super_admin', 'admin')
             OR negocio_id::text = (auth.jwt() ->> 'negocio_id'))
    $f$, r.table_schema, r.table_name);
  END LOOP;
END $$;

COMMIT;
