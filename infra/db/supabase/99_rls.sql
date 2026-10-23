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

-- ── catalog.productos ───────────────────────────────────────────────────
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

ALTER TABLE identity.empresa_miembros ENABLE ROW LEVEL SECURITY;
CREATE POLICY miembros_super_admin ON identity.empresa_miembros FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin')) WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY miembros_empresa ON identity.empresa_miembros FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY miembros_empresario_insert ON identity.empresa_miembros FOR INSERT TO authenticated
  WITH CHECK (auth.rol() = 'empresario' AND (usuario_id = auth.uid() OR
    empresa_id::text = (auth.jwt() ->> 'tenant_id')));

-- ADR-26: historial de slugs de empresa. El trigger que lo mantiene es
-- SECURITY DEFINER; las policies cubren la lectura de miembros y admin.
ALTER TABLE identity.empresa_slugs ENABLE ROW LEVEL SECURITY;
CREATE POLICY empresa_slugs_super_admin ON identity.empresa_slugs FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin')) WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY empresa_slugs_miembros ON identity.empresa_slugs FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id')
         OR empresa_id IN (SELECT id FROM identity.empresas WHERE creado_por = auth.uid()));

ALTER TABLE catalog.productos ENABLE ROW LEVEL SECURITY;
CREATE POLICY productos_admin_all ON catalog.productos
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY productos_empresa ON catalog.productos
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY productos_lectura_publica ON catalog.productos
  FOR SELECT TO anon, authenticated USING (true);

ALTER TABLE catalog.historico_precios ENABLE ROW LEVEL SECURITY;
CREATE POLICY historico_admin ON catalog.historico_precios
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');

ALTER TABLE catalog.ajustes_stock ENABLE ROW LEVEL SECURITY;
CREATE POLICY ajustes_admin ON catalog.ajustes_stock
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');

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

-- ── orders.proyeccion_ordenes ───────────────────────────────────────────
ALTER TABLE orders.proyeccion_ordenes ENABLE ROW LEVEL SECURITY;
CREATE POLICY ordenes_admin ON orders.proyeccion_ordenes
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY ordenes_comprador_propias ON orders.proyeccion_ordenes
  FOR SELECT TO authenticated
  USING (auth.rol() = 'comprador' AND cliente_id = auth.uid());
-- el vendedor ve sus ventas a traves de los items enriquecidos (items_json)
CREATE POLICY ordenes_vendedor_ventas ON orders.proyeccion_ordenes
  FOR SELECT TO authenticated
  USING (auth.rol() = 'vendedor'
         AND EXISTS (SELECT 1 FROM jsonb_array_elements(items_json) i
                     WHERE i->>'vendedor_id' = auth.uid()::text));

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
