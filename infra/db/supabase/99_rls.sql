-- Core Engine · Core Engine
-- Polticas Row Level Security (RLS) por rol (Tabla 15). Se aplican en el
-- entorno local igual que en Supabase: ejecutar este archivo con las
-- funciones auth.uid()/auth.rol() activas (Supabase las provee).
--
-- Para el entorno local de desarrollo basta con los esquemas (01_esquemas.sql);
-- este archivo se aplica en staging/produccion (Supabase) donde los servicios
-- se conectan con el rol autenticado y las claves de sesion.
--
-- Matriz de autorizacion (doc, cap. 4.3):
--   Productos:    admin CRUD completo | vendedor lectura | comprador lectura
--   Tiendas:      admin CRUD | vendedor gestion de su tienda | comprador lectura
--   Ofertas:      admin lectura | vendedor CRUD de sus ofertas | comprador lectura
--   Ordenes:      admin lectura global | vendedor solo sus ventas | comprador solo sus compras
--   Liquidaciones: admin gestion y cierre | vendedor solo las suyas | comprador --

-- ── identity.multi_vertical (ADR-0001) ─────────────────────────────────
ALTER TABLE identity.verticales ENABLE ROW LEVEL SECURITY;
CREATE POLICY verticales_lectura ON identity.verticales
  FOR SELECT TO authenticated USING (activo = true OR auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY verticales_admin ON identity.verticales
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.capacidades ENABLE ROW LEVEL SECURITY;
CREATE POLICY capacidades_lectura ON identity.capacidades
  FOR SELECT TO authenticated USING (activo = true OR auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY capacidades_admin ON identity.capacidades
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.vertical_capacidades ENABLE ROW LEVEL SECURITY;
CREATE POLICY vertical_capacidades_lectura ON identity.vertical_capacidades
  FOR SELECT TO authenticated USING (activo = true OR auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY vertical_capacidades_admin ON identity.vertical_capacidades
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.packs ENABLE ROW LEVEL SECURITY;
CREATE POLICY packs_lectura ON identity.packs
  FOR SELECT TO authenticated USING (activo = true OR auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY packs_admin ON identity.packs
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.pack_capacidades ENABLE ROW LEVEL SECURITY;
CREATE POLICY pack_capacidades_lectura ON identity.pack_capacidades
  FOR SELECT TO authenticated
  USING (EXISTS (
    SELECT 1 FROM identity.packs p
    WHERE p.codigo = pack_codigo AND p.version = pack_version AND p.activo = true
  ));
CREATE POLICY pack_capacidades_admin ON identity.pack_capacidades
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.pack_migraciones ENABLE ROW LEVEL SECURITY;
CREATE POLICY pack_migraciones_admin ON identity.pack_migraciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY pack_migraciones_empresa ON identity.pack_migraciones
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE identity.empresa_capacidades ENABLE ROW LEVEL SECURITY;
CREATE POLICY empresa_capacidades_admin ON identity.empresa_capacidades
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY empresa_capacidades_empresa ON identity.empresa_capacidades
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY empresa_capacidades_empresario_update ON identity.empresa_capacidades
  FOR UPDATE TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY empresa_capacidades_empresario_insert ON identity.empresa_capacidades
  FOR INSERT TO authenticated
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- ── identity.configuracion_plataforma (ADR-38) ─────────────────────────
-- El singleton es de plataforma, no de tenant: cualquier usuario autenticado
-- puede leer el estado público; solo los roles globales pueden modificarlo.
ALTER TABLE identity.configuracion_plataforma ENABLE ROW LEVEL SECURITY;
CREATE POLICY configuracion_plataforma_admin ON identity.configuracion_plataforma
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY configuracion_plataforma_lectura ON identity.configuracion_plataforma
  FOR SELECT TO authenticated USING (true);

-- ── identity.empresas ───────────────────────────────────────────────────
ALTER TABLE identity.empresas ENABLE ROW LEVEL SECURITY;
CREATE POLICY empresas_super_admin ON identity.empresas FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin')) WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY empresas_miembros ON identity.empresas FOR SELECT TO authenticated
  USING (id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY empresas_empresario_insert ON identity.empresas FOR INSERT TO authenticated
  WITH CHECK (auth.rol() = 'empresario' AND creado_por = auth.uid());
CREATE POLICY empresas_empresario_update ON identity.empresas FOR UPDATE TO authenticated
  USING (auth.rol() = 'empresario' AND id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND id::text = (auth.jwt() ->> 'tenant_id'));

-- ADR-26: historial de slugs de empresa. El trigger que lo mantiene es
-- SECURITY DEFINER; las policies cubren la lectura de miembros y admin.
ALTER TABLE identity.empresa_slugs ENABLE ROW LEVEL SECURITY;
CREATE POLICY empresa_slugs_super_admin ON identity.empresa_slugs FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin')) WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY empresa_slugs_miembros ON identity.empresa_slugs FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id')
         OR empresa_id IN (SELECT id FROM identity.empresas WHERE creado_por = auth.uid()));

-- ── stores.tiendas ──────────────────────────────────────────────────────
ALTER TABLE stores.tiendas ENABLE ROW LEVEL SECURITY;
CREATE POLICY tiendas_admin ON stores.tiendas
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY tiendas_vendedor_propias ON stores.tiendas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('vendedor','empresario') AND empresa_id::text = (auth.jwt() ->> 'tenant_id')
         AND (propietario_usuario_id = auth.uid() OR auth.rol() = 'empresario'))
  WITH CHECK (auth.rol() IN ('vendedor','empresario') AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY tiendas_lectura ON stores.tiendas
  FOR SELECT TO anon, authenticated USING (true);

-- ADR-26: historial de slugs de tienda. El slug es publico para el storefront,
-- pero solo el propietario (o admin) puede cambiarlo.
ALTER TABLE stores.tienda_slugs ENABLE ROW LEVEL SECURITY;
CREATE POLICY tienda_slugs_admin ON stores.tienda_slugs FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin')) WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY tienda_slugs_vendedor ON stores.tienda_slugs FOR ALL TO authenticated
  USING (auth.rol() IN ('vendedor','empresario')
         AND tienda_id IN (SELECT id FROM stores.tiendas
                           WHERE empresa_id::text = (auth.jwt() ->> 'tenant_id')))
  WITH CHECK (auth.rol() IN ('vendedor','empresario')
              AND tienda_id IN (SELECT id FROM stores.tiendas
                                WHERE empresa_id::text = (auth.jwt() ->> 'tenant_id')));
CREATE POLICY tienda_slugs_lectura ON stores.tienda_slugs
  FOR SELECT TO anon, authenticated USING (true);

-- ── stores.ofertas ──────────────────────────────────────────────────────
ALTER TABLE stores.ofertas ENABLE ROW LEVEL SECURITY;
CREATE POLICY ofertas_admin ON stores.ofertas
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY ofertas_vendedor_propias ON stores.ofertas
  FOR ALL TO authenticated
  USING (auth.rol() = 'vendedor'
         AND tienda_id IN (SELECT id FROM stores.tiendas WHERE propietario_usuario_id = auth.uid()))
  WITH CHECK (auth.rol() = 'vendedor'
              AND tienda_id IN (SELECT id FROM stores.tiendas WHERE propietario_usuario_id = auth.uid()));
CREATE POLICY ofertas_lectura ON stores.ofertas
  FOR SELECT TO anon, authenticated USING (true);

-- ── orders.orden_vista (proyección CQRS vigente) ────────────────────────
-- El código y el event sourcing escriben/leen `orden_vista`.
-- Sin estas políticas, la proyección real quedaba sin RLS en Supabase.
ALTER TABLE orders.orden_vista ENABLE ROW LEVEL SECURITY;
CREATE POLICY orden_vista_admin ON orders.orden_vista
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY orden_vista_comprador ON orders.orden_vista
  FOR SELECT TO authenticated
  USING (auth.rol() = 'comprador' AND comprador_id = auth.uid());
CREATE POLICY orden_vista_vendedor ON orders.orden_vista
  FOR SELECT TO authenticated
  USING (auth.rol() = 'vendedor'
         AND (vendedor_id = auth.uid()
              OR EXISTS (SELECT 1 FROM jsonb_array_elements(items_json) i
                         WHERE i->>'vendedor_id' = auth.uid()::text)));

-- ── orders.carritos (RN-05: cada comprador solo su carrito) ──────────────
ALTER TABLE orders.carritos ENABLE ROW LEVEL SECURITY;
CREATE POLICY carritos_comprador ON orders.carritos
  FOR ALL TO authenticated
  USING (auth.rol() = 'comprador' AND comprador_id = auth.uid())
  WITH CHECK (auth.rol() = 'comprador' AND comprador_id = auth.uid());
CREATE POLICY carritos_admin ON orders.carritos
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');

-- ── stores.eventos_procesados (idempotencia interna, sin acceso publico) ─
ALTER TABLE stores.eventos_procesados ENABLE ROW LEVEL SECURITY;
CREATE POLICY eventos_procesados_admin ON stores.eventos_procesados
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');

-- ── commissions ─────────────────────────────────────────────────────────
ALTER TABLE commissions.comisiones ENABLE ROW LEVEL SECURITY;
CREATE POLICY comisiones_admin ON commissions.comisiones
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY comisiones_vendedor_propias ON commissions.comisiones
  FOR SELECT TO authenticated
  USING (auth.rol() = 'vendedor' AND vendedor_id = auth.uid());

ALTER TABLE commissions.liquidaciones ENABLE ROW LEVEL SECURITY;
CREATE POLICY liquidaciones_admin ON commissions.liquidaciones
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY liquidaciones_vendedor_propias ON commissions.liquidaciones
  FOR SELECT TO authenticated
  USING (auth.rol() = 'vendedor' AND vendedor_id = auth.uid());

ALTER TABLE commissions.liquidaciones_empresas ENABLE ROW LEVEL SECURITY;
CREATE POLICY liquidaciones_empresas_admin ON commissions.liquidaciones_empresas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY liquidaciones_empresario_propias ON commissions.liquidaciones_empresas
  FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE commissions.pagos ENABLE ROW LEVEL SECURITY;
CREATE POLICY pagos_admin ON commissions.pagos
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');

-- ── logistics.envios ────────────────────────────────────────────────────
ALTER TABLE logistics.envios ENABLE ROW LEVEL SECURITY;
CREATE POLICY envios_admin ON logistics.envios
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY envios_logistica ON logistics.envios
  FOR ALL TO authenticated
  USING (auth.rol() IN ('logistica', 'coordinador', 'supervisor', 'operativo')
         AND proveedor_tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('logistica', 'coordinador', 'supervisor', 'operativo')
              AND proveedor_tenant_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY envios_empresario ON logistics.envios
  FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE logistics.asignaciones_envio ENABLE ROW LEVEL SECURITY;
CREATE POLICY asignaciones_envio_admin ON logistics.asignaciones_envio
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY asignaciones_envio_empresa ON logistics.asignaciones_envio
  FOR SELECT TO authenticated
  USING (EXISTS (
    SELECT 1 FROM logistics.envios e
    WHERE e.id = envio_id AND e.empresa_id::text = (auth.jwt() ->> 'tenant_id')
  ));

-- ── finance.workspace_registros (operación de comercio) ─────────────────
-- Cada fila pertenece a una empresa (tenant_id) o a un vendedor
-- (propietario_id). El frontend de empresa y el de vendedor comparten el mismo
-- recurso, así que la política separa ambos ámbitos sin mezclar carteras.
ALTER TABLE finance.workspace_registros ENABLE ROW LEVEL SECURITY;
CREATE POLICY workspace_registros_admin ON finance.workspace_registros
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY workspace_registros_empresa ON finance.workspace_registros
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY workspace_registros_vendedor ON finance.workspace_registros
  FOR ALL TO authenticated
  USING (auth.rol() = 'vendedor' AND propietario_id::text = auth.uid())
  WITH CHECK (auth.rol() = 'vendedor' AND propietario_id::text = auth.uid());

-- ── scheduling (plataforma multi-vertical, fase 4) ─────────────────────
ALTER TABLE scheduling.recursos ENABLE ROW LEVEL SECURITY;
CREATE POLICY scheduling_recursos_admin ON scheduling.recursos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_recursos_empresa ON scheduling.recursos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor')
        AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor')
             AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE scheduling.disponibilidad ENABLE ROW LEVEL SECURITY;
CREATE POLICY scheduling_disponibilidad_admin ON scheduling.disponibilidad
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_disponibilidad_empresa ON scheduling.disponibilidad
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor')
        AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor')
             AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE scheduling.bloqueos ENABLE ROW LEVEL SECURITY;
CREATE POLICY scheduling_bloqueos_admin ON scheduling.bloqueos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_bloqueos_empresa ON scheduling.bloqueos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor')
        AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor')
             AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE scheduling.citas ENABLE ROW LEVEL SECURITY;
CREATE POLICY scheduling_citas_admin ON scheduling.citas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_citas_empresa ON scheduling.citas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor', 'operativo')
        AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor', 'operativo')
             AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE scheduling.recordatorios ENABLE ROW LEVEL SECURITY;
CREATE POLICY scheduling_recordatorios_admin ON scheduling.recordatorios
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY scheduling_recordatorios_empresa ON scheduling.recordatorios
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'coordinador', 'supervisor', 'operativo')
        AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'coordinador', 'supervisor', 'operativo')
             AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE scheduling.eventos_procesados ENABLE ROW LEVEL SECURITY;
CREATE POLICY scheduling_eventos_procesados_admin ON scheduling.eventos_procesados
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE scheduling.outbox ENABLE ROW LEVEL SECURITY;
CREATE POLICY scheduling_outbox_admin ON scheduling.outbox
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.modelos_operacion ENABLE ROW LEVEL SECURITY;
CREATE POLICY modelos_operacion_lectura ON identity.modelos_operacion
  FOR SELECT TO anon, authenticated USING (activo);
CREATE POLICY modelos_operacion_admin ON identity.modelos_operacion
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.pack_modelos ENABLE ROW LEVEL SECURITY;
CREATE POLICY pack_modelos_lectura ON identity.pack_modelos
  FOR SELECT TO authenticated
  USING (EXISTS (
    SELECT 1 FROM identity.packs p
     WHERE p.codigo = pack_codigo AND p.version = pack_version AND p.activo
  ));
CREATE POLICY pack_modelos_admin ON identity.pack_modelos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE identity.extensiones_empresa ENABLE ROW LEVEL SECURITY;
CREATE POLICY extensiones_empresa_admin ON identity.extensiones_empresa
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY extensiones_empresa_lectura ON identity.extensiones_empresa
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extensiones_empresa_insert ON identity.extensiones_empresa
  FOR INSERT TO authenticated
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extensiones_empresa_update ON identity.extensiones_empresa
  FOR UPDATE TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE identity.extensiones_empresa_valores ENABLE ROW LEVEL SECURITY;
CREATE POLICY extension_values_admin ON identity.extensiones_empresa_valores
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY extension_values_lectura ON identity.extensiones_empresa_valores
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extension_values_insert ON identity.extensiones_empresa_valores
  FOR INSERT TO authenticated
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extension_values_update ON identity.extensiones_empresa_valores
  FOR UPDATE TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY extension_values_delete ON identity.extensiones_empresa_valores
  FOR DELETE TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE identity.partes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS partes_admin ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_lectura ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_escritura ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_insert ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_update ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_delete ON identity.partes;
DROP POLICY IF EXISTS partes_empresa_archivar ON identity.partes;
CREATE POLICY partes_admin ON identity.partes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY partes_empresa_lectura ON identity.partes FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_empresa_insert ON identity.partes FOR INSERT TO authenticated
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_empresa_update ON identity.partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_empresa_delete ON identity.partes FOR DELETE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_empresa_archivar ON identity.partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NULL)
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id')
         AND eliminado_en IS NOT NULL AND estado = 'archivado');

-- Cuentas de crédito por parte (venta a crédito desde checkout).
ALTER TABLE finance.credito_cuentas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS credito_cuentas_admin ON finance.credito_cuentas;
DROP POLICY IF EXISTS credito_cuentas_lectura ON finance.credito_cuentas;
DROP POLICY IF EXISTS credito_cuentas_escritura ON finance.credito_cuentas;
CREATE POLICY credito_cuentas_admin ON finance.credito_cuentas FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY credito_cuentas_lectura ON finance.credito_cuentas FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY credito_cuentas_escritura ON finance.credito_cuentas FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- Registros genéricos por parte (historia clínica, recetas, expedientes).
ALTER TABLE identity.partes_registros ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS partes_registros_admin ON identity.partes_registros;
DROP POLICY IF EXISTS partes_registros_lectura ON identity.partes_registros;
DROP POLICY IF EXISTS partes_registros_insert ON identity.partes_registros;
DROP POLICY IF EXISTS partes_registros_update ON identity.partes_registros;
DROP POLICY IF EXISTS partes_registros_delete ON identity.partes_registros;
CREATE POLICY partes_registros_admin ON identity.partes_registros FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY partes_registros_lectura ON identity.partes_registros FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_registros_insert ON identity.partes_registros FOR INSERT TO authenticated
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_registros_update ON identity.partes_registros FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY partes_registros_delete ON identity.partes_registros FOR DELETE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE identity.medios_contacto ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS medios_contacto_admin ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_lectura ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_escritura ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_insert ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_update ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_delete ON identity.medios_contacto;
DROP POLICY IF EXISTS medios_contacto_empresa_archivar ON identity.medios_contacto;
CREATE POLICY medios_contacto_admin ON identity.medios_contacto FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY medios_contacto_empresa_lectura ON identity.medios_contacto FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY medios_contacto_empresa_insert ON identity.medios_contacto FOR INSERT TO authenticated
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY medios_contacto_empresa_update ON identity.medios_contacto FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY medios_contacto_empresa_delete ON identity.medios_contacto FOR DELETE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY medios_contacto_empresa_archivar ON identity.medios_contacto FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NULL)
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NOT NULL);

ALTER TABLE identity.direcciones_partes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS direcciones_partes_admin ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_lectura ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_escritura ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_insert ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_update ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_delete ON identity.direcciones_partes;
DROP POLICY IF EXISTS direcciones_partes_empresa_archivar ON identity.direcciones_partes;
CREATE POLICY direcciones_partes_admin ON identity.direcciones_partes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY direcciones_partes_empresa_lectura ON identity.direcciones_partes FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY direcciones_partes_empresa_insert ON identity.direcciones_partes FOR INSERT TO authenticated
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY direcciones_partes_empresa_update ON identity.direcciones_partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY direcciones_partes_empresa_delete ON identity.direcciones_partes FOR DELETE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY direcciones_partes_empresa_archivar ON identity.direcciones_partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NULL)
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND eliminado_en IS NOT NULL);

ALTER TABLE identity.relaciones_partes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS relaciones_partes_admin ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_lectura ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_escritura ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_insert ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_update ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_delete ON identity.relaciones_partes;
DROP POLICY IF EXISTS relaciones_partes_empresa_desactivar ON identity.relaciones_partes;
CREATE POLICY relaciones_partes_admin ON identity.relaciones_partes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY relaciones_partes_empresa_lectura ON identity.relaciones_partes FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY relaciones_partes_empresa_insert ON identity.relaciones_partes FOR INSERT TO authenticated
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY relaciones_partes_empresa_update ON identity.relaciones_partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY relaciones_partes_empresa_delete ON identity.relaciones_partes FOR DELETE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY relaciones_partes_empresa_desactivar ON identity.relaciones_partes FOR UPDATE TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND activo)
  WITH CHECK (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id') AND NOT activo);

-- Personal configurable por empresa: permisos, puestos, alcance e incorporación.
ALTER TABLE identity.roles_empresa ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS roles_empresa_admin ON identity.roles_empresa;
DROP POLICY IF EXISTS roles_empresa_tenant ON identity.roles_empresa;
CREATE POLICY roles_empresa_admin ON identity.roles_empresa FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY roles_empresa_tenant ON identity.roles_empresa FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY roles_empresa_app ON identity.roles_empresa FOR SELECT TO core_engine_app
  USING (true);

ALTER TABLE identity.puestos_empresa ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS puestos_empresa_admin ON identity.puestos_empresa;
DROP POLICY IF EXISTS puestos_empresa_tenant ON identity.puestos_empresa;
CREATE POLICY puestos_empresa_admin ON identity.puestos_empresa FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY puestos_empresa_tenant ON identity.puestos_empresa FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE identity.asignaciones_personal ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS asignaciones_personal_admin ON identity.asignaciones_personal;
DROP POLICY IF EXISTS asignaciones_personal_tenant ON identity.asignaciones_personal;
CREATE POLICY asignaciones_personal_admin ON identity.asignaciones_personal FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY asignaciones_personal_tenant ON identity.asignaciones_personal FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE identity.invitaciones_personal ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS invitaciones_personal_admin ON identity.invitaciones_personal;
DROP POLICY IF EXISTS invitaciones_personal_tenant ON identity.invitaciones_personal;
DROP POLICY IF EXISTS invitaciones_personal_app ON identity.invitaciones_personal;
CREATE POLICY invitaciones_personal_admin ON identity.invitaciones_personal FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY invitaciones_personal_tenant ON identity.invitaciones_personal FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
-- El servicio valida token, estado, expiración y tenant antes de devolver datos.
CREATE POLICY invitaciones_personal_app ON identity.invitaciones_personal FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

-- ── field (logística de campo / piloto flota) ───────────────────────────
-- Aísla cada tabla por tenant_id; eventos/outbox quedan para administración.
ALTER TABLE field.personal ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_personal_admin ON field.personal;
CREATE POLICY field_personal_admin ON field.personal
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_personal_tenant ON field.personal;
CREATE POLICY field_personal_tenant ON field.personal
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.clientes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_clientes_admin ON field.clientes;
CREATE POLICY field_clientes_admin ON field.clientes
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_clientes_tenant ON field.clientes;
CREATE POLICY field_clientes_tenant ON field.clientes
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.vehiculos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_vehiculos_admin ON field.vehiculos;
CREATE POLICY field_vehiculos_admin ON field.vehiculos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_vehiculos_tenant ON field.vehiculos;
CREATE POLICY field_vehiculos_tenant ON field.vehiculos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.rutas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_rutas_admin ON field.rutas;
CREATE POLICY field_rutas_admin ON field.rutas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_rutas_tenant ON field.rutas;
CREATE POLICY field_rutas_tenant ON field.rutas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.paradas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_paradas_admin ON field.paradas;
CREATE POLICY field_paradas_admin ON field.paradas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_paradas_tenant ON field.paradas;
CREATE POLICY field_paradas_tenant ON field.paradas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.pedidos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_pedidos_admin ON field.pedidos;
CREATE POLICY field_pedidos_admin ON field.pedidos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_pedidos_tenant ON field.pedidos;
CREATE POLICY field_pedidos_tenant ON field.pedidos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.asistencia ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_asistencia_admin ON field.asistencia;
CREATE POLICY field_asistencia_admin ON field.asistencia
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_asistencia_tenant ON field.asistencia;
CREATE POLICY field_asistencia_tenant ON field.asistencia
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.incidencias ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_incidencias_admin ON field.incidencias;
CREATE POLICY field_incidencias_admin ON field.incidencias
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_incidencias_tenant ON field.incidencias;
CREATE POLICY field_incidencias_tenant ON field.incidencias
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.tracking ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_tracking_admin ON field.tracking;
CREATE POLICY field_tracking_admin ON field.tracking
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_tracking_tenant ON field.tracking;
CREATE POLICY field_tracking_tenant ON field.tracking
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.visitas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_visitas_admin ON field.visitas;
CREATE POLICY field_visitas_admin ON field.visitas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_visitas_tenant ON field.visitas;
CREATE POLICY field_visitas_tenant ON field.visitas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE field.cumplimiento ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS field_cumplimiento_admin ON field.cumplimiento;
CREATE POLICY field_cumplimiento_admin ON field.cumplimiento
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS field_cumplimiento_tenant ON field.cumplimiento;
CREATE POLICY field_cumplimiento_tenant ON field.cumplimiento
  FOR ALL TO authenticated
  USING (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
         AND tenant_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() IN ('empresario', 'logistica', 'coordinador', 'supervisor', 'operativo')
              AND tenant_id::text = (auth.jwt() ->> 'tenant_id'));
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

-- ── identity.password_reset_tokens (ADR-0003) ───────────────────────────────
-- El token es una capacidad de un solo uso: solo el rol de servicio de identity
-- lo lee y consume. No se expone a `authenticated` (ni a admin/empresario).
ALTER TABLE identity.password_reset_tokens ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS password_reset_tokens_app ON identity.password_reset_tokens;
CREATE POLICY password_reset_tokens_app ON identity.password_reset_tokens
  FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

-- ── notifications.eventos_procesados (ADR-0003) ─────────────────────────────
-- Idempotencia del consumidor: solo el rol de servicio accede.
ALTER TABLE notifications.eventos_procesados ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS notifications_eventos_app ON notifications.eventos_procesados;
CREATE POLICY notifications_eventos_app ON notifications.eventos_procesados
  FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

-- ── identity.api_keys (ADR-0005/R-GW-8) ─────────────────────────────────────
-- Tabla de credenciales de máquina: solo el rol de servicio de identity la lee,
-- crea y revoca. El aislamiento por tenant lo aplica el servicio (empresa_id).
ALTER TABLE identity.api_keys ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS api_keys_app ON identity.api_keys;
CREATE POLICY api_keys_app ON identity.api_keys
  FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

-- ── orders.entregas_digitales (P2.1) ────────────────────────────────────────
ALTER TABLE orders.entregas_digitales ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS entregas_digitales_admin ON orders.entregas_digitales;
CREATE POLICY entregas_digitales_admin ON orders.entregas_digitales
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS entregas_digitales_comprador ON orders.entregas_digitales;
CREATE POLICY entregas_digitales_comprador ON orders.entregas_digitales
  FOR SELECT TO authenticated
  USING (auth.rol() = 'comprador' AND comprador_id = auth.uid());
DROP POLICY IF EXISTS entregas_digitales_app ON orders.entregas_digitales;
CREATE POLICY entregas_digitales_app ON orders.entregas_digitales
  FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

-- ── orders.contratos y contrato_eventos (P2.2) ──────────────────────────────
ALTER TABLE orders.contratos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS contratos_admin ON orders.contratos;
CREATE POLICY contratos_admin ON orders.contratos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS contratos_empresa ON orders.contratos;
CREATE POLICY contratos_empresa ON orders.contratos
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS contratos_lectura ON orders.contratos;
CREATE POLICY contratos_lectura ON orders.contratos
  FOR SELECT TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE orders.contrato_eventos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS contrato_eventos_admin ON orders.contrato_eventos;
CREATE POLICY contrato_eventos_admin ON orders.contrato_eventos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS contrato_eventos_empresa ON orders.contrato_eventos;
CREATE POLICY contrato_eventos_empresa ON orders.contrato_eventos
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS contrato_eventos_lectura ON orders.contrato_eventos;
CREATE POLICY contrato_eventos_lectura ON orders.contrato_eventos
  FOR SELECT TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- ── orders.suscripciones y suscripcion_ejecuciones (P2.3) ───────────────────
ALTER TABLE orders.suscripciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS suscripciones_admin ON orders.suscripciones;
CREATE POLICY suscripciones_admin ON orders.suscripciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS suscripciones_empresa ON orders.suscripciones;
CREATE POLICY suscripciones_empresa ON orders.suscripciones
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS suscripciones_comprador ON orders.suscripciones;
CREATE POLICY suscripciones_comprador ON orders.suscripciones
  FOR SELECT TO authenticated
  USING (auth.rol() = 'comprador' AND comprador_id = auth.uid());

ALTER TABLE orders.suscripcion_ejecuciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS suscripcion_ejecuciones_admin ON orders.suscripcion_ejecuciones;
CREATE POLICY suscripcion_ejecuciones_admin ON orders.suscripcion_ejecuciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS suscripcion_ejecuciones_empresa ON orders.suscripcion_ejecuciones;
CREATE POLICY suscripcion_ejecuciones_empresa ON orders.suscripcion_ejecuciones
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS suscripcion_ejecuciones_comprador ON orders.suscripcion_ejecuciones;
CREATE POLICY suscripcion_ejecuciones_comprador ON orders.suscripcion_ejecuciones
  FOR SELECT TO authenticated
  USING (auth.rol() = 'comprador' AND empresa_id::text IN (
    SELECT empresa_id::text FROM orders.suscripciones s
     WHERE s.comprador_id = auth.uid()
  ));

-- ── orders.mesas y orders.comandas (P2.5) ───────────────────────────────────
ALTER TABLE orders.mesas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS mesas_admin ON orders.mesas;
CREATE POLICY mesas_admin ON orders.mesas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS mesas_empresa ON orders.mesas;
CREATE POLICY mesas_empresa ON orders.mesas
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS mesas_lectura ON orders.mesas;
CREATE POLICY mesas_lectura ON orders.mesas
  FOR SELECT TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE orders.comandas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS comandas_admin ON orders.comandas;
CREATE POLICY comandas_admin ON orders.comandas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS comandas_empresa ON orders.comandas;
CREATE POLICY comandas_empresa ON orders.comandas
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS comandas_lectura ON orders.comandas;
CREATE POLICY comandas_lectura ON orders.comandas
  FOR SELECT TO authenticated
  USING (auth.rol() IN ('empresario', 'vendedor', 'coordinador', 'supervisor', 'operativo')
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- ── inventory (ADR-0018) ────────────────────────────────────────────────────
ALTER TABLE inventory.configuracion_inventario ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS configuracion_inventario_admin ON inventory.configuracion_inventario;
CREATE POLICY configuracion_inventario_admin ON inventory.configuracion_inventario
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS configuracion_inventario_empresa ON inventory.configuracion_inventario;
CREATE POLICY configuracion_inventario_empresa ON inventory.configuracion_inventario
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.proveedores ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS proveedores_admin ON inventory.proveedores;
CREATE POLICY proveedores_admin ON inventory.proveedores
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS proveedores_empresa ON inventory.proveedores;
CREATE POLICY proveedores_empresa ON inventory.proveedores
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.lotes_inventario ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS lotes_inventario_admin ON inventory.lotes_inventario;
CREATE POLICY lotes_inventario_admin ON inventory.lotes_inventario
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS lotes_inventario_empresa ON inventory.lotes_inventario;
CREATE POLICY lotes_inventario_empresa ON inventory.lotes_inventario
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.ubicaciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS ubicaciones_admin ON inventory.ubicaciones;
CREATE POLICY ubicaciones_admin ON inventory.ubicaciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS ubicaciones_empresa ON inventory.ubicaciones;
CREATE POLICY ubicaciones_empresa ON inventory.ubicaciones
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.item_stock_ubicacion ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_stock_ubicacion_admin ON inventory.item_stock_ubicacion;
CREATE POLICY item_stock_ubicacion_admin ON inventory.item_stock_ubicacion
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS item_stock_ubicacion_empresa ON inventory.item_stock_ubicacion;
CREATE POLICY item_stock_ubicacion_empresa ON inventory.item_stock_ubicacion
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.movimientos_inventario ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS movimientos_inventario_admin ON inventory.movimientos_inventario;
CREATE POLICY movimientos_inventario_admin ON inventory.movimientos_inventario
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS movimientos_inventario_empresa ON inventory.movimientos_inventario;
CREATE POLICY movimientos_inventario_empresa ON inventory.movimientos_inventario
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.ajustes_stock ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS ajustes_stock_admin ON inventory.ajustes_stock;
CREATE POLICY ajustes_stock_admin ON inventory.ajustes_stock
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS ajustes_stock_empresa ON inventory.ajustes_stock;
CREATE POLICY ajustes_stock_empresa ON inventory.ajustes_stock
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.reservas_lotes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS reservas_lotes_admin ON inventory.reservas_lotes;
CREATE POLICY reservas_lotes_admin ON inventory.reservas_lotes
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS reservas_lotes_empresa ON inventory.reservas_lotes;
CREATE POLICY reservas_lotes_empresa ON inventory.reservas_lotes
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE inventory.reservas_ordenes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS reservas_ordenes_admin ON inventory.reservas_ordenes;
CREATE POLICY reservas_ordenes_admin ON inventory.reservas_ordenes
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE inventory.eventos_procesados ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS eventos_procesados_admin ON inventory.eventos_procesados;
CREATE POLICY eventos_procesados_admin ON inventory.eventos_procesados
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE inventory.outbox ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS outbox_admin ON inventory.outbox;
CREATE POLICY outbox_admin ON inventory.outbox
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
-- El poller del Outbox corre sin claims de usuario; el rol de aplicación debe
-- poder reclamar y publicar los pendientes.
DROP POLICY IF EXISTS outbox_app ON inventory.outbox;
CREATE POLICY outbox_app ON inventory.outbox
  FOR ALL TO core_engine_app
  USING (true)
  WITH CHECK (true);

-- El poller del Outbox corre sin claims de usuario; el rol de aplicación debe
-- poder reclamar y publicar los pendientes.
DROP POLICY IF EXISTS outbox_app ON inventory.outbox;
CREATE POLICY outbox_app ON inventory.outbox
  FOR ALL TO core_engine_app
  USING (true)
  WITH CHECK (true);

-- ── orders.metodos_pago y orders.pagos (Fase 2 · pagos) ─────────────────────
ALTER TABLE orders.metodos_pago ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS metodos_pago_lectura ON orders.metodos_pago;
CREATE POLICY metodos_pago_lectura ON orders.metodos_pago
  FOR SELECT TO authenticated USING (activo);
DROP POLICY IF EXISTS metodos_pago_admin ON orders.metodos_pago;
CREATE POLICY metodos_pago_admin ON orders.metodos_pago
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE orders.pagos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS pagos_admin ON orders.pagos;
CREATE POLICY pagos_admin ON orders.pagos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS pagos_empresa ON orders.pagos;
CREATE POLICY pagos_empresa ON orders.pagos
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- ── orders.estados_orden, tipos_orden y transiciones (Fase 3) ───────────────
ALTER TABLE orders.estados_orden ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS estados_orden_lectura ON orders.estados_orden;
CREATE POLICY estados_orden_lectura ON orders.estados_orden
  FOR SELECT TO authenticated USING (true);
DROP POLICY IF EXISTS estados_orden_admin ON orders.estados_orden;
CREATE POLICY estados_orden_admin ON orders.estados_orden
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE orders.tipos_orden ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tipos_orden_lectura ON orders.tipos_orden;
CREATE POLICY tipos_orden_lectura ON orders.tipos_orden
  FOR SELECT TO authenticated USING (activo);
DROP POLICY IF EXISTS tipos_orden_admin ON orders.tipos_orden;
CREATE POLICY tipos_orden_admin ON orders.tipos_orden
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

ALTER TABLE orders.transiciones_orden ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS transiciones_orden_lectura ON orders.transiciones_orden;
CREATE POLICY transiciones_orden_lectura ON orders.transiciones_orden
  FOR SELECT TO authenticated USING (true);
DROP POLICY IF EXISTS transiciones_orden_admin ON orders.transiciones_orden;
CREATE POLICY transiciones_orden_admin ON orders.transiciones_orden
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
-- ── pricing (ADR-0018) ──────────────────────────────────────────────────────
ALTER TABLE pricing.listas_precio ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS listas_precio_admin ON pricing.listas_precio;
CREATE POLICY listas_precio_admin ON pricing.listas_precio FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS listas_precio_empresa ON pricing.listas_precio;
CREATE POLICY listas_precio_empresa ON pricing.listas_precio FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE pricing.lista_precio_items ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS lista_precio_items_admin ON pricing.lista_precio_items;
CREATE POLICY lista_precio_items_admin ON pricing.lista_precio_items FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS lista_precio_items_empresa ON pricing.lista_precio_items;
CREATE POLICY lista_precio_items_empresa ON pricing.lista_precio_items FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE pricing.item_tarifas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_tarifas_admin ON pricing.item_tarifas;
CREATE POLICY item_tarifas_admin ON pricing.item_tarifas FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS item_tarifas_empresa ON pricing.item_tarifas;
CREATE POLICY item_tarifas_empresa ON pricing.item_tarifas FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE pricing.promociones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS promociones_admin ON pricing.promociones;
CREATE POLICY promociones_admin ON pricing.promociones FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS promociones_empresa ON pricing.promociones;
CREATE POLICY promociones_empresa ON pricing.promociones FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE pricing.outbox ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS outbox_admin ON pricing.outbox;
CREATE POLICY outbox_admin ON pricing.outbox FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS outbox_app ON pricing.outbox;
CREATE POLICY outbox_app ON pricing.outbox FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

-- ── orders.fulfillments (Fase 4) ────────────────────────────────────────────
ALTER TABLE orders.fulfillments ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS fulfillments_lectura ON orders.fulfillments;
CREATE POLICY fulfillments_lectura ON orders.fulfillments
  FOR SELECT TO authenticated USING (activo);
DROP POLICY IF EXISTS fulfillments_admin ON orders.fulfillments;
CREATE POLICY fulfillments_admin ON orders.fulfillments
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
-- ── item (ADR-0018) ─────────────────────────────────────────────────────────
DO $$
DECLARE tabla text;
BEGIN
  FOREACH tabla IN ARRAY ARRAY['items','item_atributo_esquemas','item_imagenes','item_componentes','item_series','historico_precios'] LOOP
    EXECUTE format('ALTER TABLE item.%I ENABLE ROW LEVEL SECURITY', tabla);
    EXECUTE format('DROP POLICY IF EXISTS %I ON item.%I', tabla || '_admin', tabla);
    EXECUTE format('CREATE POLICY %I ON item.%I FOR ALL TO authenticated USING (auth.rol() IN (''super_admin'', ''admin'')) WITH CHECK (auth.rol() IN (''super_admin'', ''admin''))', tabla || '_admin', tabla);
    EXECUTE format('DROP POLICY IF EXISTS %I ON item.%I', tabla || '_empresa', tabla);
    EXECUTE format('CREATE POLICY %I ON item.%I FOR ALL TO authenticated USING (auth.rol() = ''empresario'' AND empresa_id::text = (auth.jwt() ->> ''tenant_id'')) WITH CHECK (auth.rol() = ''empresario'' AND empresa_id::text = (auth.jwt() ->> ''tenant_id''))', tabla || '_empresa', tabla);
  END LOOP;
END $$;

ALTER TABLE item.outbox ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS outbox_admin ON item.outbox;
DROP POLICY IF EXISTS outbox_app ON item.outbox;
CREATE POLICY outbox_admin ON item.outbox FOR ALL TO authenticated USING (auth.rol() IN ('super_admin','admin')) WITH CHECK (auth.rol() IN ('super_admin','admin'));
CREATE POLICY outbox_app ON item.outbox FOR ALL TO core_engine_app USING (true) WITH CHECK (true);
ALTER TABLE item.items ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS items_admin ON item.items;
CREATE POLICY items_admin ON item.items FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin','admin'))
  WITH CHECK (auth.rol() IN ('super_admin','admin'));
DROP POLICY IF EXISTS items_empresa ON item.items;
CREATE POLICY items_empresa ON item.items FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));



ALTER TABLE item.item_atributo_esquemas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_atributo_esquemas_admin ON item.item_atributo_esquemas;
CREATE POLICY item_atributo_esquemas_admin ON item.item_atributo_esquemas FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin','admin'))
  WITH CHECK (auth.rol() IN ('super_admin','admin'));
DROP POLICY IF EXISTS item_atributo_esquemas_empresa ON item.item_atributo_esquemas;
CREATE POLICY item_atributo_esquemas_empresa ON item.item_atributo_esquemas FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE item.item_imagenes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_imagenes_admin ON item.item_imagenes;
CREATE POLICY item_imagenes_admin ON item.item_imagenes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin','admin'))
  WITH CHECK (auth.rol() IN ('super_admin','admin'));
DROP POLICY IF EXISTS item_imagenes_empresa ON item.item_imagenes;
CREATE POLICY item_imagenes_empresa ON item.item_imagenes FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE item.item_componentes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_componentes_admin ON item.item_componentes;
CREATE POLICY item_componentes_admin ON item.item_componentes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin','admin'))
  WITH CHECK (auth.rol() IN ('super_admin','admin'));
DROP POLICY IF EXISTS item_componentes_empresa ON item.item_componentes;
CREATE POLICY item_componentes_empresa ON item.item_componentes FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE item.item_series ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS item_series_admin ON item.item_series;
CREATE POLICY item_series_admin ON item.item_series FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin','admin'))
  WITH CHECK (auth.rol() IN ('super_admin','admin'));
DROP POLICY IF EXISTS item_series_empresa ON item.item_series;
CREATE POLICY item_series_empresa ON item.item_series FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));


ALTER TABLE pricing.historico_precios_base ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS historico_precios_base_admin ON pricing.historico_precios_base;
CREATE POLICY historico_precios_base_admin ON pricing.historico_precios_base FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin','admin'))
  WITH CHECK (auth.rol() IN ('super_admin','admin'));
DROP POLICY IF EXISTS historico_precios_base_empresa ON pricing.historico_precios_base;
CREATE POLICY historico_precios_base_empresa ON pricing.historico_precios_base FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- ── identity.registros_esquemas (Fase 5) ────────────────────────────────────
ALTER TABLE identity.registros_esquemas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS registros_esquemas_admin ON identity.registros_esquemas;
CREATE POLICY registros_esquemas_admin ON identity.registros_esquemas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS registros_esquemas_empresa ON identity.registros_esquemas;
CREATE POLICY registros_esquemas_empresa ON identity.registros_esquemas
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- ── finance.tipos_cambio (Fase 7) ───────────────────────────────────────────
ALTER TABLE finance.tipos_cambio ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS tipos_cambio_lectura ON finance.tipos_cambio;
CREATE POLICY tipos_cambio_lectura ON finance.tipos_cambio
  FOR SELECT TO authenticated USING (true);
DROP POLICY IF EXISTS tipos_cambio_admin ON finance.tipos_cambio;
CREATE POLICY tipos_cambio_admin ON finance.tipos_cambio
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));

-- ── orders.fidelizacion_* (Fase 8) ──────────────────────────────────────────
ALTER TABLE orders.fidelizacion_programas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS fidelizacion_programas_admin ON orders.fidelizacion_programas;
CREATE POLICY fidelizacion_programas_admin ON orders.fidelizacion_programas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS fidelizacion_programas_empresa ON orders.fidelizacion_programas;
CREATE POLICY fidelizacion_programas_empresa ON orders.fidelizacion_programas
  FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE orders.fidelizacion_tarjetas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS fidelizacion_tarjetas_admin ON orders.fidelizacion_tarjetas;
CREATE POLICY fidelizacion_tarjetas_admin ON orders.fidelizacion_tarjetas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS fidelizacion_tarjetas_empresa ON orders.fidelizacion_tarjetas;
CREATE POLICY fidelizacion_tarjetas_empresa ON orders.fidelizacion_tarjetas
  FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE orders.fidelizacion_movimientos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS fidelizacion_movimientos_admin ON orders.fidelizacion_movimientos;
CREATE POLICY fidelizacion_movimientos_admin ON orders.fidelizacion_movimientos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS fidelizacion_movimientos_empresa ON orders.fidelizacion_movimientos;
CREATE POLICY fidelizacion_movimientos_empresa ON orders.fidelizacion_movimientos
  FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE orders.fidelizacion_recompensas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS fidelizacion_recompensas_admin ON orders.fidelizacion_recompensas;
CREATE POLICY fidelizacion_recompensas_admin ON orders.fidelizacion_recompensas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS fidelizacion_recompensas_empresa ON orders.fidelizacion_recompensas;
CREATE POLICY fidelizacion_recompensas_empresa ON orders.fidelizacion_recompensas
  FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE orders.fidelizacion_canjes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS fidelizacion_canjes_admin ON orders.fidelizacion_canjes;
CREATE POLICY fidelizacion_canjes_admin ON orders.fidelizacion_canjes
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS fidelizacion_canjes_empresa ON orders.fidelizacion_canjes;
CREATE POLICY fidelizacion_canjes_empresa ON orders.fidelizacion_canjes
  FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
-- ── identity.negocios (ADR-0021) ─────────────────────────────────────────
-- Un negocio pertenece a una empresa; el aislamiento sigue siendo por tenant.
ALTER TABLE identity.negocios ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS negocios_admin ON identity.negocios;
DROP POLICY IF EXISTS negocios_tenant ON identity.negocios;
DROP POLICY IF EXISTS negocios_app ON identity.negocios;
CREATE POLICY negocios_admin ON identity.negocios FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY negocios_tenant ON identity.negocios FOR ALL TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario'
          AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY negocios_app ON identity.negocios FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

-- ── identity.negocio_capacidades (ADR-0021 fase 3) ───────────────────────
ALTER TABLE identity.negocio_capacidades ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS negocio_capacidades_admin ON identity.negocio_capacidades;
DROP POLICY IF EXISTS negocio_capacidades_tenant ON identity.negocio_capacidades;
DROP POLICY IF EXISTS negocio_capacidades_app ON identity.negocio_capacidades;
CREATE POLICY negocio_capacidades_admin ON identity.negocio_capacidades
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY negocio_capacidades_tenant ON identity.negocio_capacidades
  FOR ALL TO authenticated
  USING (EXISTS (
    SELECT 1 FROM identity.negocios n
     WHERE n.id = negocio_id
       AND n.empresa_id::text = (auth.jwt() ->> 'tenant_id')
  ))
  WITH CHECK (auth.rol() = 'empresario' AND EXISTS (
    SELECT 1 FROM identity.negocios n
     WHERE n.id = negocio_id
       AND n.empresa_id::text = (auth.jwt() ->> 'tenant_id')
  ));
CREATE POLICY negocio_capacidades_app ON identity.negocio_capacidades
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── identity.permisos (ADR-0021 iteración 7) ─────────────────────────────
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

-- ── RLS restrictiva por negocio (ADR-0021 iteración 8) ───────────────────
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

-- ── inventory por negocio (ADR-0021 iteración 9) ─────────────────────────
ALTER TABLE inventory.ubicaciones ADD COLUMN IF NOT EXISTS negocio_id uuid;
ALTER TABLE inventory.proveedores ADD COLUMN IF NOT EXISTS negocio_id uuid;
ALTER TABLE inventory.configuracion_inventario ADD COLUMN IF NOT EXISTS negocio_id uuid;

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

-- ── finance.series_comprobantes por negocio (ADR-0021 iteración 10) ──────
ALTER TABLE finance.series_comprobantes ADD COLUMN IF NOT EXISTS negocio_id uuid;
ALTER TABLE finance.series_comprobantes ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS series_comprobantes_admin ON finance.series_comprobantes;
DROP POLICY IF EXISTS series_comprobantes_tenant ON finance.series_comprobantes;
DROP POLICY IF EXISTS series_comprobantes_negocio_restrictiva ON finance.series_comprobantes;
CREATE POLICY series_comprobantes_admin ON finance.series_comprobantes FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY series_comprobantes_tenant ON finance.series_comprobantes FOR SELECT TO authenticated
  USING (true);
CREATE POLICY series_comprobantes_negocio_restrictiva ON finance.series_comprobantes
  AS RESTRICTIVE FOR ALL TO authenticated
  USING (auth.negocio_actual() IS NULL
         OR negocio_id IS NULL
         OR negocio_id::text = auth.negocio_actual())
  WITH CHECK (auth.negocio_actual() IS NULL
         OR negocio_id IS NULL
         OR negocio_id::text = auth.negocio_actual());

-- ── identity.auditoria (ADR-0021 iteración 11) ───────────────────────────
ALTER TABLE identity.auditoria ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS auditoria_admin ON identity.auditoria;
DROP POLICY IF EXISTS auditoria_tenant ON identity.auditoria;
DROP POLICY IF EXISTS auditoria_app ON identity.auditoria;
CREATE POLICY auditoria_admin ON identity.auditoria FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY auditoria_tenant ON identity.auditoria FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id')
         AND (auth.negocio_actual() IS NULL
              OR negocio_id IS NULL
              OR negocio_id::text = auth.negocio_actual()));
CREATE POLICY auditoria_app ON identity.auditoria FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

-- ── identity.plantillas_roles (ADR-0021 iteración 12) ─────────────────────
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

-- ── analytics.* (ADR-0021/0020: analytics-service) ──────────────────────
ALTER TABLE analytics.hechos_venta ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS hechos_venta_admin ON analytics.hechos_venta;
DROP POLICY IF EXISTS hechos_venta_empresa ON analytics.hechos_venta;
DROP POLICY IF EXISTS hechos_venta_app ON analytics.hechos_venta;
CREATE POLICY hechos_venta_admin ON analytics.hechos_venta FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY hechos_venta_empresa ON analytics.hechos_venta FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario'
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY hechos_venta_app ON analytics.hechos_venta FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

ALTER TABLE analytics.comisiones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS comisiones_admin ON analytics.comisiones;
DROP POLICY IF EXISTS comisiones_empresa ON analytics.comisiones;
DROP POLICY IF EXISTS comisiones_app ON analytics.comisiones;
CREATE POLICY comisiones_admin ON analytics.comisiones FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY comisiones_empresa ON analytics.comisiones FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario'
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY comisiones_app ON analytics.comisiones FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

ALTER TABLE analytics.eventos_procesados ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS eventos_procesados_admin ON analytics.eventos_procesados;
DROP POLICY IF EXISTS eventos_procesados_app ON analytics.eventos_procesados;
CREATE POLICY eventos_procesados_admin ON analytics.eventos_procesados FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY eventos_procesados_app ON analytics.eventos_procesados FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

ALTER TABLE analytics.reportes_programados ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS reportes_programados_admin ON analytics.reportes_programados;
DROP POLICY IF EXISTS reportes_programados_empresa ON analytics.reportes_programados;
DROP POLICY IF EXISTS reportes_programados_app ON analytics.reportes_programados;
CREATE POLICY reportes_programados_admin ON analytics.reportes_programados FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY reportes_programados_empresa ON analytics.reportes_programados FOR ALL TO authenticated
  USING (auth.rol() = 'empresario'
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario'
              AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY reportes_programados_app ON analytics.reportes_programados FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

ALTER TABLE analytics.reporte_ejecuciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS reporte_ejecuciones_admin ON analytics.reporte_ejecuciones;
DROP POLICY IF EXISTS reporte_ejecuciones_empresa ON analytics.reporte_ejecuciones;
DROP POLICY IF EXISTS reporte_ejecuciones_app ON analytics.reporte_ejecuciones;
CREATE POLICY reporte_ejecuciones_admin ON analytics.reporte_ejecuciones FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY reporte_ejecuciones_empresa ON analytics.reporte_ejecuciones FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario'
         AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY reporte_ejecuciones_app ON analytics.reporte_ejecuciones FOR ALL TO core_engine_app
  USING (true) WITH CHECK (true);

-- ── finance.configuracion_fiscal y orders.pagos_eventos (ADR-0026/0027) ─────
ALTER TABLE finance.configuracion_fiscal ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS configuracion_fiscal_admin ON finance.configuracion_fiscal;
CREATE POLICY configuracion_fiscal_admin ON finance.configuracion_fiscal
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS configuracion_fiscal_empresa ON finance.configuracion_fiscal;
CREATE POLICY configuracion_fiscal_empresa ON finance.configuracion_fiscal
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS configuracion_fiscal_app ON finance.configuracion_fiscal;
CREATE POLICY configuracion_fiscal_app ON finance.configuracion_fiscal
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE orders.pagos_eventos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS pagos_eventos_admin ON orders.pagos_eventos;
CREATE POLICY pagos_eventos_admin ON orders.pagos_eventos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS pagos_eventos_empresa ON orders.pagos_eventos;
CREATE POLICY pagos_eventos_empresa ON orders.pagos_eventos
  FOR SELECT TO authenticated
  USING (EXISTS (
    SELECT 1 FROM orders.pagos p
     WHERE p.id = pago_id AND p.empresa_id::text = (auth.jwt() ->> 'tenant_id')
  ));
DROP POLICY IF EXISTS pagos_eventos_app ON orders.pagos_eventos;
CREATE POLICY pagos_eventos_app ON orders.pagos_eventos
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── pricing.cupones y pricing.cupon_usos (fase retail) ─────────────────────
ALTER TABLE pricing.cupones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS cupones_admin ON pricing.cupones;
CREATE POLICY cupones_admin ON pricing.cupones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS cupones_empresa ON pricing.cupones;
CREATE POLICY cupones_empresa ON pricing.cupones
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS cupones_app ON pricing.cupones;
CREATE POLICY cupones_app ON pricing.cupones
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE pricing.cupon_usos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS cupon_usos_admin ON pricing.cupon_usos;
CREATE POLICY cupon_usos_admin ON pricing.cupon_usos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS cupon_usos_empresa ON pricing.cupon_usos;
CREATE POLICY cupon_usos_empresa ON pricing.cupon_usos
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
DROP POLICY IF EXISTS cupon_usos_app ON pricing.cupon_usos;
CREATE POLICY cupon_usos_app ON pricing.cupon_usos
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── orders.devoluciones y orders.devolucion_lineas (RMA) ───────────────────
ALTER TABLE orders.devoluciones ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS devoluciones_admin ON orders.devoluciones;
CREATE POLICY devoluciones_admin ON orders.devoluciones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS devoluciones_comprador ON orders.devoluciones;
CREATE POLICY devoluciones_comprador ON orders.devoluciones
  FOR SELECT TO authenticated
  USING (EXISTS (
    SELECT 1 FROM orders.orden_vista o
     WHERE o.id = order_id AND o.comprador_id::text = (auth.jwt() ->> 'sub')
  ));
DROP POLICY IF EXISTS devoluciones_app ON orders.devoluciones;
CREATE POLICY devoluciones_app ON orders.devoluciones
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE orders.devolucion_lineas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS devolucion_lineas_admin ON orders.devolucion_lineas;
CREATE POLICY devolucion_lineas_admin ON orders.devolucion_lineas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS devolucion_lineas_app ON orders.devolucion_lineas;
CREATE POLICY devolucion_lineas_app ON orders.devolucion_lineas
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE inventory.devoluciones_procesadas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS devoluciones_procesadas_app ON inventory.devoluciones_procesadas;
CREATE POLICY devoluciones_procesadas_app ON inventory.devoluciones_procesadas
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── stores.resenas y stores.favoritos (reseñas/wishlist) ───────────────────
ALTER TABLE stores.resenas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS resenas_publicas ON stores.resenas;
CREATE POLICY resenas_publicas ON stores.resenas
  FOR SELECT TO authenticated USING (estado = 'publicada');
DROP POLICY IF EXISTS resenas_comprador ON stores.resenas;
CREATE POLICY resenas_comprador ON stores.resenas
  FOR ALL TO authenticated
  USING (comprador_id::text = (auth.jwt() ->> 'sub'))
  WITH CHECK (comprador_id::text = (auth.jwt() ->> 'sub'));
DROP POLICY IF EXISTS resenas_admin ON stores.resenas;
CREATE POLICY resenas_admin ON stores.resenas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS resenas_app ON stores.resenas;
CREATE POLICY resenas_app ON stores.resenas
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE stores.favoritos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS favoritos_comprador ON stores.favoritos;
CREATE POLICY favoritos_comprador ON stores.favoritos
  FOR ALL TO authenticated
  USING (comprador_id::text = (auth.jwt() ->> 'sub'))
  WITH CHECK (comprador_id::text = (auth.jwt() ->> 'sub'));
DROP POLICY IF EXISTS favoritos_admin ON stores.favoritos;
CREATE POLICY favoritos_admin ON stores.favoritos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
DROP POLICY IF EXISTS favoritos_app ON stores.favoritos;
CREATE POLICY favoritos_app ON stores.favoritos
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── orders.idempotencia_http (ADR-0033) ────────────────────────────────────
ALTER TABLE orders.idempotencia_http ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS idempotencia_http_app ON orders.idempotencia_http;
CREATE POLICY idempotencia_http_app ON orders.idempotencia_http
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── identity.solicitudes_eliminacion_tenant (ADR-0035) ─────────────────────
ALTER TABLE identity.solicitudes_eliminacion_tenant ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS solicitudes_eliminacion_app
  ON identity.solicitudes_eliminacion_tenant;
CREATE POLICY solicitudes_eliminacion_app
  ON identity.solicitudes_eliminacion_tenant
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── orders proveedores/configuración de pago (ADR-0037) ───────────────────
ALTER TABLE orders.proveedores_pago ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS proveedores_pago_app ON orders.proveedores_pago;
CREATE POLICY proveedores_pago_app ON orders.proveedores_pago
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE orders.configuracion_pagos ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS configuracion_pagos_app ON orders.configuracion_pagos;
CREATE POLICY configuracion_pagos_app ON orders.configuracion_pagos
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── notifications.plantillas (ADR-0043) ───────────────────────────────────
ALTER TABLE notifications.plantillas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS notificaciones_plantillas_app ON notifications.plantillas;
CREATE POLICY notificaciones_plantillas_app ON notifications.plantillas
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── identity.exportacion_jobs (ADR-0042) ──────────────────────────────────
ALTER TABLE identity.exportacion_jobs ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS exportacion_jobs_app ON identity.exportacion_jobs;
CREATE POLICY exportacion_jobs_app ON identity.exportacion_jobs
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── finance.comprobante_pdf (ADR-0040) ────────────────────────────────────
ALTER TABLE finance.comprobante_pdf ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS comprobante_pdf_app ON finance.comprobante_pdf;
CREATE POLICY comprobante_pdf_app ON finance.comprobante_pdf
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── identity auth hardening (ADR-0036) ────────────────────────────────────
ALTER TABLE identity.correo_verificacion_tokens ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS correo_verificacion_app ON identity.correo_verificacion_tokens;
CREATE POLICY correo_verificacion_app ON identity.correo_verificacion_tokens
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE identity.mfa_codigos_recuperacion ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS mfa_codigos_app ON identity.mfa_codigos_recuperacion;
CREATE POLICY mfa_codigos_app ON identity.mfa_codigos_recuperacion
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── identity.planes_plataforma y suscripciones (ADR-0046) ─────────────────
ALTER TABLE identity.planes_plataforma ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS planes_plataforma_app ON identity.planes_plataforma;
CREATE POLICY planes_plataforma_app ON identity.planes_plataforma
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE identity.suscripciones_plataforma ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS suscripciones_plataforma_app ON identity.suscripciones_plataforma;
CREATE POLICY suscripciones_plataforma_app ON identity.suscripciones_plataforma
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

-- ── notifications.webhooks y entregas (ADR-0045) ──────────────────────────
ALTER TABLE notifications.webhooks ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS notifications_webhooks_app ON notifications.webhooks;
CREATE POLICY notifications_webhooks_app ON notifications.webhooks
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);

ALTER TABLE notifications.webhook_entregas ENABLE ROW LEVEL SECURITY;
DROP POLICY IF EXISTS notifications_webhook_entregas_app ON notifications.webhook_entregas;
CREATE POLICY notifications_webhook_entregas_app ON notifications.webhook_entregas
  FOR ALL TO core_engine_app USING (true) WITH CHECK (true);
