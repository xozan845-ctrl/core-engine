-- Plataforma multi-vertical · RLS de las superficies nuevas.
-- Idempotente para local/staging: reemplaza solo las politicas de tablas
-- introducidas en las fases 1–4 y conserva el aislamiento por empresa.
BEGIN;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT USAGE ON SCHEMA identity, catalog, scheduling TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA identity TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA catalog TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA scheduling TO core_engine_app;
  END IF;
END $$;

-- identity: catalogos de plataforma y capacidades por empresa.
ALTER TABLE identity.verticales ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS verticales_lectura ON identity.verticales;
DROP POLICY IF EXISTS verticales_admin ON identity.verticales;
CREATE POLICY verticales_lectura ON identity.verticales FOR SELECT TO anon, authenticated USING (activo);
CREATE POLICY verticales_admin ON identity.verticales FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.capacidades ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS capacidades_lectura ON identity.capacidades;
DROP POLICY IF EXISTS capacidades_admin ON identity.capacidades;
CREATE POLICY capacidades_lectura ON identity.capacidades FOR SELECT TO anon, authenticated USING (activo);
CREATE POLICY capacidades_admin ON identity.capacidades FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.vertical_capacidades ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS vertical_capacidades_lectura ON identity.vertical_capacidades;
DROP POLICY IF EXISTS vertical_capacidades_admin ON identity.vertical_capacidades;
CREATE POLICY vertical_capacidades_lectura ON identity.vertical_capacidades FOR SELECT TO anon, authenticated USING (activo);
CREATE POLICY vertical_capacidades_admin ON identity.vertical_capacidades FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.packs ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS packs_lectura ON identity.packs;
DROP POLICY IF EXISTS packs_admin ON identity.packs;
CREATE POLICY packs_lectura ON identity.packs FOR SELECT TO authenticated USING (activo);
CREATE POLICY packs_admin ON identity.packs FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.pack_capacidades ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS pack_capacidades_lectura ON identity.pack_capacidades;
DROP POLICY IF EXISTS pack_capacidades_admin ON identity.pack_capacidades;
CREATE POLICY pack_capacidades_lectura ON identity.pack_capacidades FOR SELECT TO authenticated
  USING (EXISTS (SELECT 1 FROM identity.packs p WHERE p.codigo = pack_codigo AND p.version = pack_version AND p.activo));
CREATE POLICY pack_capacidades_admin ON identity.pack_capacidades FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.empresa_capacidades ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS empresa_capacidades_admin ON identity.empresa_capacidades;
DROP POLICY IF EXISTS empresa_capacidades_empresa ON identity.empresa_capacidades;
DROP POLICY IF EXISTS empresa_capacidades_empresario_update ON identity.empresa_capacidades;
DROP POLICY IF EXISTS empresa_capacidades_empresario_insert ON identity.empresa_capacidades;
CREATE POLICY empresa_capacidades_admin ON identity.empresa_capacidades FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY empresa_capacidades_empresa ON identity.empresa_capacidades FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY empresa_capacidades_empresario_update ON identity.empresa_capacidades FOR UPDATE TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY empresa_capacidades_empresario_insert ON identity.empresa_capacidades FOR INSERT TO authenticated
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE identity.pack_migraciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS pack_migraciones_admin ON identity.pack_migraciones;
DROP POLICY IF EXISTS pack_migraciones_empresa ON identity.pack_migraciones;
CREATE POLICY pack_migraciones_admin ON identity.pack_migraciones FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY pack_migraciones_empresa ON identity.pack_migraciones FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- catalog: item unificado, esquemas, proveedores y lotes.
ALTER TABLE catalog.items ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS items_admin ON catalog.items;
DROP POLICY IF EXISTS items_empresa ON catalog.items;
CREATE POLICY items_admin ON catalog.items FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY items_empresa ON catalog.items FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY items_empresa_lectura ON catalog.items FOR SELECT TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.item_atributo_esquemas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_schemas_admin ON catalog.item_atributo_esquemas;
DROP POLICY IF EXISTS item_schemas_lectura ON catalog.item_atributo_esquemas;
DROP POLICY IF EXISTS item_schemas_empresa ON catalog.item_atributo_esquemas;
CREATE POLICY item_schemas_admin ON catalog.item_atributo_esquemas FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY item_schemas_lectura ON catalog.item_atributo_esquemas FOR SELECT TO authenticated
  USING (empresa_id IS NULL OR empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY item_schemas_empresa ON catalog.item_atributo_esquemas FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.proveedores ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS proveedores_admin ON catalog.proveedores;
DROP POLICY IF EXISTS proveedores_empresa ON catalog.proveedores;
CREATE POLICY proveedores_admin ON catalog.proveedores FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY proveedores_empresa ON catalog.proveedores FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.item_proveedores ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_proveedores_admin ON catalog.item_proveedores;
DROP POLICY IF EXISTS item_proveedores_empresa ON catalog.item_proveedores;
CREATE POLICY item_proveedores_admin ON catalog.item_proveedores FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY item_proveedores_empresa ON catalog.item_proveedores FOR ALL TO authenticated
  USING (EXISTS (SELECT 1 FROM catalog.items i WHERE i.id = item_id AND i.empresa_id::text = (auth.jwt() ->> 'tenant_id')))
  WITH CHECK (EXISTS (SELECT 1 FROM catalog.items i WHERE i.id = item_id AND i.empresa_id::text = (auth.jwt() ->> 'tenant_id')));

ALTER TABLE catalog.lotes_inventario ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS lotes_admin ON catalog.lotes_inventario;
DROP POLICY IF EXISTS lotes_empresa ON catalog.lotes_inventario;
CREATE POLICY lotes_admin ON catalog.lotes_inventario FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY lotes_empresa ON catalog.lotes_inventario FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- scheduling: bounded context con aislamiento por empresa y soporte de jobs
-- internos (eventos procesados/outbox solo para roles administrativos).
DO $$
DECLARE tabla text;
BEGIN
  FOREACH tabla IN ARRAY ARRAY['recursos','disponibilidad','bloqueos','citas','recordatorios','eventos_procesados','outbox'] LOOP
    EXECUTE format('ALTER TABLE scheduling.%I ENABLE ROW LEVEL SECURITY', tabla);
  END LOOP;
END $$;

DROP POLICY IF EXISTS scheduling_recursos_admin ON scheduling.recursos;
DROP POLICY IF EXISTS scheduling_recursos_empresa ON scheduling.recursos;
CREATE POLICY scheduling_recursos_admin ON scheduling.recursos FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_recursos_empresa ON scheduling.recursos FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

DROP POLICY IF EXISTS scheduling_disponibilidad_admin ON scheduling.disponibilidad;
DROP POLICY IF EXISTS scheduling_disponibilidad_empresa ON scheduling.disponibilidad;
CREATE POLICY scheduling_disponibilidad_admin ON scheduling.disponibilidad FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_disponibilidad_empresa ON scheduling.disponibilidad FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

DROP POLICY IF EXISTS scheduling_bloqueos_admin ON scheduling.bloqueos;
DROP POLICY IF EXISTS scheduling_bloqueos_empresa ON scheduling.bloqueos;
CREATE POLICY scheduling_bloqueos_admin ON scheduling.bloqueos FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_bloqueos_empresa ON scheduling.bloqueos FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

DROP POLICY IF EXISTS scheduling_citas_admin ON scheduling.citas;
DROP POLICY IF EXISTS scheduling_citas_empresa ON scheduling.citas;
CREATE POLICY scheduling_citas_admin ON scheduling.citas FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_citas_empresa ON scheduling.citas FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor', 'operativo') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor', 'operativo') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

DROP POLICY IF EXISTS scheduling_recordatorios_admin ON scheduling.recordatorios;
DROP POLICY IF EXISTS scheduling_recordatorios_empresa ON scheduling.recordatorios;
CREATE POLICY scheduling_recordatorios_admin ON scheduling.recordatorios FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_recordatorios_empresa ON scheduling.recordatorios FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor', 'operativo') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor', 'operativo') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

DROP POLICY IF EXISTS scheduling_eventos_procesados_admin ON scheduling.eventos_procesados;
CREATE POLICY scheduling_eventos_procesados_admin ON scheduling.eventos_procesados FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

DROP POLICY IF EXISTS scheduling_outbox_admin ON scheduling.outbox;
DROP POLICY IF EXISTS scheduling_outbox_empresa_insert ON scheduling.outbox;
CREATE POLICY scheduling_outbox_admin ON scheduling.outbox FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_outbox_empresa_insert ON scheduling.outbox FOR INSERT TO authenticated
  WITH CHECK (
    auth.rol() IN ('empresario', 'coordinador', 'supervisor', 'operativo')
    AND payload -> 'data' ->> 'empresa_id' = (auth.jwt() ->> 'tenant_id')
  );

-- El catálogo público se consulta sin JWT. La API mantiene el alcance por
-- empresa_id y solo expone productos activos; RLS debe permitir esa lectura
-- anónima sin abrir escrituras ni productos despublicados.
ALTER TABLE catalog.producto_catalogo_vista ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS catalogo_vista_public_read ON catalog.producto_catalogo_vista;
CREATE POLICY catalogo_vista_public_read ON catalog.producto_catalogo_vista
  FOR SELECT TO public
  USING (estado_operativo = 'activo');

COMMIT;
