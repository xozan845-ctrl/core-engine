-- Endurecimiento posterior a 99_rls.sql (R-SB-2, ADR-0054).
--
-- 1. Elimina las políticas `TO core_engine_app USING (true)` que actuaban como
--    bypass de RLS. El rol de aplicación conserva NOSUPERUSER/NOBYPASSRLS.
-- 2. Habilita RLS en todas las tablas de aplicación que aún no lo tenían.
-- 3. Crea una política de tenant (empresa/negocio) o admin-only si la tabla no
--    tiene columna de tenant.
-- 4. Añade políticas específicas para usuarios, sesiones, timeline de pedidos
--    y asientos contables.
-- 5. Activa FORCE ROW LEVEL SECURITY para que el owner tampoco lo omita.
--
-- Idempotente: se puede aplicar dos veces.
BEGIN;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    EXECUTE 'ALTER ROLE core_engine_app NOSUPERUSER NOBYPASSRLS';
  END IF;
END $$;

-- 1. Quitar bypass del rol de aplicación. Outbox/eventos se tratan aparte para
--    que el relay siga funcionando con claims de servicio (super_admin).
DO $$
DECLARE r record;
BEGIN
  FOR r IN
    SELECT p.schemaname, p.tablename, p.policyname
      FROM pg_policies p
     WHERE 'core_engine_app' = ANY (p.roles)
       AND p.qual = 'true'
       AND p.tablename NOT LIKE '%outbox'
       AND p.tablename NOT LIKE '%eventos_procesados'
  LOOP
    EXECUTE format('DROP POLICY IF EXISTS %I ON %I.%I', r.policyname, r.schemaname, r.tablename);
  END LOOP;
END $$;

-- 2. Outbox y eventos procesados: solo administración/servicio con claims.
DO $$
DECLARE r record;
DECLARE r2 record;
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
       AND (c.table_name LIKE '%outbox' OR c.table_name LIKE '%eventos_procesados')
  LOOP
    EXECUTE format('ALTER TABLE %I.%I ENABLE ROW LEVEL SECURITY', r.table_schema, r.table_name);
    -- Quitar el bypass histórico del rol de aplicación.
    FOR r2 IN
      SELECT p.policyname
        FROM pg_policies p
       WHERE p.schemaname = r.table_schema
         AND p.tablename = r.table_name
         AND 'core_engine_app' = ANY (p.roles)
         AND p.qual = 'true'
    LOOP
      EXECUTE format('DROP POLICY IF EXISTS %I ON %I.%I', r2.policyname, r.table_schema, r.table_name);
    END LOOP;
    EXECUTE format('DROP POLICY IF EXISTS rls_hardening_service ON %I.%I', r.table_schema, r.table_name);
    EXECUTE format(
      'CREATE POLICY rls_hardening_service ON %I.%I FOR ALL TO authenticated
         USING (auth.rol() IN (''super_admin'', ''admin''))
         WITH CHECK (auth.rol() IN (''super_admin'', ''admin''))',
      r.table_schema, r.table_name
    );
  END LOOP;
END $$;

-- 3. RLS y política base para el resto de tablas de aplicación.
DO $$
DECLARE
  r record;
  tiene_empresa boolean;
  tiene_tenant boolean;
  tiene_negocio boolean;
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
  LOOP
    EXECUTE format('ALTER TABLE %I.%I ENABLE ROW LEVEL SECURITY', r.table_schema, r.table_name);

    SELECT EXISTS (
             SELECT 1 FROM information_schema.columns col
              WHERE col.table_schema = r.table_schema
                AND col.table_name = r.table_name
                AND col.column_name = 'empresa_id'
           ),
           EXISTS (
             SELECT 1 FROM information_schema.columns col
              WHERE col.table_schema = r.table_schema
                AND col.table_name = r.table_name
                AND col.column_name = 'tenant_id'
           ),
           EXISTS (
             SELECT 1 FROM information_schema.columns col
              WHERE col.table_schema = r.table_schema
                AND col.table_name = r.table_name
                AND col.column_name = 'negocio_id'
           )
      INTO tiene_empresa, tiene_tenant, tiene_negocio;

    EXECUTE format('DROP POLICY IF EXISTS rls_hardening_tenant ON %I.%I', r.table_schema, r.table_name);
    EXECUTE format('DROP POLICY IF EXISTS rls_hardening_admin ON %I.%I', r.table_schema, r.table_name);

    IF tiene_empresa THEN
      EXECUTE format($f$
        CREATE POLICY rls_hardening_tenant ON %I.%I FOR ALL TO authenticated
        USING (auth.rol() IN ('super_admin', 'admin')
               OR empresa_id::text = (auth.jwt() ->> 'tenant_id'))
        WITH CHECK (auth.rol() IN ('super_admin', 'admin')
               OR empresa_id::text = (auth.jwt() ->> 'tenant_id'))
      $f$, r.table_schema, r.table_name);
    ELSIF tiene_tenant THEN
      EXECUTE format($f$
        CREATE POLICY rls_hardening_tenant ON %I.%I FOR ALL TO authenticated
        USING (auth.rol() IN ('super_admin', 'admin')
               OR tenant_id::text = (auth.jwt() ->> 'tenant_id'))
        WITH CHECK (auth.rol() IN ('super_admin', 'admin')
               OR tenant_id::text = (auth.jwt() ->> 'tenant_id'))
      $f$, r.table_schema, r.table_name);
    ELSIF tiene_negocio THEN
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
    ELSE
      EXECUTE format($f$
        CREATE POLICY rls_hardening_admin ON %I.%I FOR ALL TO authenticated
        USING (auth.rol() IN ('super_admin', 'admin'))
        WITH CHECK (auth.rol() IN ('super_admin', 'admin'))
      $f$, r.table_schema, r.table_name);
    END IF;
  END LOOP;
END $$;

-- 4. Políticas específicas para las tablas sensibles del cierre.
DO $$
BEGIN
  -- Ofertas: el catálogo público solo expone ofertas activas.
  EXECUTE 'DROP POLICY IF EXISTS ofertas_lectura ON stores.ofertas';
  EXECUTE $f$
    CREATE POLICY ofertas_lectura ON stores.ofertas FOR SELECT TO anon, authenticated
    USING (estado = 'activa')
  $f$;

  -- Series fiscales: cada tenant ve solo sus series.
  EXECUTE 'DROP POLICY IF EXISTS series_comprobantes_tenant ON finance.series_comprobantes';
  EXECUTE $f$
    CREATE POLICY series_comprobantes_tenant ON finance.series_comprobantes
    FOR SELECT TO authenticated
    USING (auth.rol() IN ('super_admin', 'admin')
           OR empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  $f$;

  -- Usuarios: el propio usuario, su tenant o admin.
  EXECUTE 'ALTER TABLE identity.usuarios ENABLE ROW LEVEL SECURITY';
  EXECUTE 'DROP POLICY IF EXISTS usuarios_self ON identity.usuarios';
  EXECUTE $f$
    CREATE POLICY usuarios_self ON identity.usuarios FOR SELECT TO authenticated
    USING (id = auth.uid())
  $f$;

  -- Sesiones: solo el propio usuario o admin/servicio.
  EXECUTE 'ALTER TABLE identity.sesiones ENABLE ROW LEVEL SECURITY';
  EXECUTE 'DROP POLICY IF EXISTS sesiones_self ON identity.sesiones';
  EXECUTE $f$
    CREATE POLICY sesiones_self ON identity.sesiones FOR SELECT TO authenticated
    USING (usuario_id = auth.uid()
           OR auth.rol() IN ('super_admin', 'admin'))
  $f$;
  EXECUTE 'DROP POLICY IF EXISTS sesiones_update_self ON identity.sesiones';
  EXECUTE $f$
    CREATE POLICY sesiones_update_self ON identity.sesiones FOR UPDATE TO authenticated
    USING (usuario_id = auth.uid()
           OR auth.rol() IN ('super_admin', 'admin'))
    WITH CHECK (usuario_id = auth.uid()
                OR auth.rol() IN ('super_admin', 'admin'))
  $f$;

  -- Timeline: visible si el pedido pertenece al tenant.
  EXECUTE 'ALTER TABLE orders.orden_timeline ENABLE ROW LEVEL SECURITY';
  EXECUTE 'DROP POLICY IF EXISTS orden_timeline_tenant ON orders.orden_timeline';
  EXECUTE $f$
    CREATE POLICY orden_timeline_tenant ON orders.orden_timeline FOR SELECT TO authenticated
    USING (auth.rol() IN ('super_admin', 'admin')
           OR EXISTS (
                SELECT 1 FROM orders.orden_vista o
                  JOIN stores.tiendas t ON t.id = o.tienda_id
                 WHERE o.id = order_id
                   AND t.empresa_id::text = (auth.jwt() ->> 'tenant_id')
              ))
  $f$;

  -- Asientos: visibles si su referencia es un pedido del tenant.
  EXECUTE 'ALTER TABLE finance.asientos ENABLE ROW LEVEL SECURITY';
  EXECUTE 'DROP POLICY IF EXISTS asientos_tenant ON finance.asientos';
  EXECUTE $f$
    CREATE POLICY asientos_tenant ON finance.asientos FOR SELECT TO authenticated
    USING (auth.rol() IN ('super_admin', 'admin')
           OR EXISTS (
                SELECT 1 FROM orders.orden_vista o
                  JOIN stores.tiendas t ON t.id = o.tienda_id
                 WHERE o.id::text = referencia_id
                   AND t.empresa_id::text = (auth.jwt() ->> 'tenant_id')
              ))
  $f$;
  EXECUTE 'ALTER TABLE finance.asiento_detalles ENABLE ROW LEVEL SECURITY';
  EXECUTE 'DROP POLICY IF EXISTS asiento_detalles_tenant ON finance.asiento_detalles';
  EXECUTE $f$
    CREATE POLICY asiento_detalles_tenant ON finance.asiento_detalles FOR SELECT TO authenticated
    USING (auth.rol() IN ('super_admin', 'admin')
           OR EXISTS (
                SELECT 1 FROM finance.asientos a
                  JOIN orders.orden_vista o ON o.id::text = a.referencia_id
                  JOIN stores.tiendas t ON t.id = o.tienda_id
                 WHERE a.id = asiento_id
                   AND t.empresa_id::text = (auth.jwt() ->> 'tenant_id')
              ))
  $f$;
END $$;


-- 4a. Triggers de historial de slugs (SECURITY DEFINER) escriben como owner.
-- El owner es código de confianza; el rol de app sigue aislado por tenant.
DO $$
BEGIN
  EXECUTE format('DROP POLICY IF EXISTS empresa_slugs_owner ON identity.empresa_slugs');
  EXECUTE format(
    'CREATE POLICY empresa_slugs_owner ON identity.empresa_slugs FOR ALL TO %I USING (true) WITH CHECK (true)',
    current_user
  );
  EXECUTE format('DROP POLICY IF EXISTS tienda_slugs_owner ON stores.tienda_slugs');
  EXECUTE format(
    'CREATE POLICY tienda_slugs_owner ON stores.tienda_slugs FOR ALL TO %I USING (true) WITH CHECK (true)',
    current_user
  );
END $$;

-- 4b. Catálogos fiscales de plataforma que los tenants deben poder leer.
DO $$
BEGIN
  EXECUTE 'ALTER TABLE finance.jurisdicciones ENABLE ROW LEVEL SECURITY';
  EXECUTE 'DROP POLICY IF EXISTS jurisdicciones_lectura ON finance.jurisdicciones';
  EXECUTE $f$
    CREATE POLICY jurisdicciones_lectura ON finance.jurisdicciones
    FOR SELECT TO anon, authenticated USING (activa)
  $f$;

  EXECUTE 'ALTER TABLE finance.regimenes_fiscales ENABLE ROW LEVEL SECURITY';
  EXECUTE 'DROP POLICY IF EXISTS regimenes_fiscales_lectura ON finance.regimenes_fiscales';
  EXECUTE $f$
    CREATE POLICY regimenes_fiscales_lectura ON finance.regimenes_fiscales
    FOR SELECT TO anon, authenticated USING (estado = 'activo')
  $f$;
END $$;

-- 4c. El creador ve la empresa que acaba de crear.
-- El onboarding usa `INSERT ... RETURNING` y el empresario aún no tiene
-- `tenant_id` en su token; sin esta lectura, RLS rechaza el RETURNING.
DROP POLICY IF EXISTS empresas_miembros ON identity.empresas;
CREATE POLICY empresas_miembros ON identity.empresas FOR SELECT TO authenticated
  USING (id::text = (auth.jwt() ->> 'tenant_id')
         OR (auth.rol() = 'empresario' AND creado_por = auth.uid()));

-- 4d. Binding inicial del empresario con la empresa que acaba de crear.
-- El onboarding actualiza `tenant_id` del usuario recién registrado; sin esta
-- política el UPDATE afecta 0 filas en silencio y el token no lleva tenant.
DROP POLICY IF EXISTS usuarios_onboarding_tenant ON identity.usuarios;
CREATE POLICY usuarios_onboarding_tenant ON identity.usuarios FOR UPDATE TO authenticated
  USING (auth.rol() = 'empresario'
         AND id = auth.uid()
         AND tenant_id IS NULL)
  WITH CHECK (auth.rol() = 'empresario'
              AND EXISTS (
                    SELECT 1 FROM identity.empresas e
                     WHERE e.id = tenant_id
                       AND e.creado_por = auth.uid()
                  ));

-- 4e. Historial de precios de ofertas: lo escribe el vendedor dueño de la
-- oferta (y el servicio/admin). Sin esto, publicar una oferta falla con RLS.
DROP POLICY IF EXISTS historico_precios_oferta ON stores.historico_precios;
CREATE POLICY historico_precios_oferta ON stores.historico_precios FOR ALL TO authenticated
  USING (EXISTS (
    SELECT 1
      FROM stores.ofertas o
      JOIN stores.tiendas t ON t.id = o.tienda_id
     WHERE o.id = oferta_id
       AND (auth.rol() IN ('super_admin', 'admin')
            OR t.propietario_usuario_id = auth.uid()
            OR t.empresa_id::text = (auth.jwt() ->> 'tenant_id'))))
  WITH CHECK (EXISTS (
    SELECT 1
      FROM stores.ofertas o
      JOIN stores.tiendas t ON t.id = o.tienda_id
     WHERE o.id = oferta_id
       AND (auth.rol() IN ('super_admin', 'admin')
            OR t.propietario_usuario_id = auth.uid()
            OR t.empresa_id::text = (auth.jwt() ->> 'tenant_id'))));

-- 5. FORCE RLS: ni el owner omite las políticas.
DO $$
DECLARE r record;
BEGIN
  FOR r IN
    SELECT schemaname, tablename
      FROM pg_tables
     WHERE schemaname = ANY (ARRAY[
       'identity','stores','orders','catalog','item','inventory','pricing',
       'commissions','finance','logistics','scheduling','field','notifications',
       'storage','analytics','intelligence'
     ])
       AND rowsecurity
  LOOP
    EXECUTE format('ALTER TABLE %I.%I FORCE ROW LEVEL SECURITY', r.schemaname, r.tablename);
  END LOOP;
END $$;

COMMIT;
