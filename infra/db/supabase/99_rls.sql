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
-- ADR-27: el catálogo público se sirve por el API Gateway con el alcance de la
-- empresa solicitada; no hay lectura anónima directa de la tabla.

-- ADR-28: imágenes de producto/variante. El objeto binario vive en storage;
-- esta tabla solo relaciona y se aísla por empresa.
ALTER TABLE catalog.producto_imagenes ENABLE ROW LEVEL SECURITY;
CREATE POLICY producto_imagenes_admin ON catalog.producto_imagenes
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY producto_imagenes_empresa ON catalog.producto_imagenes
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.historico_precios ENABLE ROW LEVEL SECURITY;
CREATE POLICY historico_admin ON catalog.historico_precios
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY historico_empresa ON catalog.historico_precios
  FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.ajustes_stock ENABLE ROW LEVEL SECURITY;
CREATE POLICY ajustes_admin ON catalog.ajustes_stock
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY ajustes_empresa ON catalog.ajustes_stock
  FOR SELECT TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.producto_catalogo_vista ENABLE ROW LEVEL SECURITY;
CREATE POLICY catalogo_vista_admin ON catalog.producto_catalogo_vista
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY catalogo_vista_empresa ON catalog.producto_catalogo_vista
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.stock_vista ENABLE ROW LEVEL SECURITY;
CREATE POLICY stock_vista_admin ON catalog.stock_vista
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY stock_vista_empresa ON catalog.stock_vista
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.inventario_mensual_mv ENABLE ROW LEVEL SECURITY;
CREATE POLICY inventario_mensual_admin ON catalog.inventario_mensual_mv
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');
CREATE POLICY inventario_mensual_empresa ON catalog.inventario_mensual_mv
  FOR SELECT TO authenticated
  USING (empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.reservas_ordenes ENABLE ROW LEVEL SECURITY;
CREATE POLICY reservas_admin ON catalog.reservas_ordenes
  FOR ALL TO authenticated USING (auth.rol() = 'admin') WITH CHECK (auth.rol() = 'admin');

-- ── catalog.items (plataforma multi-vertical, fase 2) ───────────────────
ALTER TABLE catalog.items ENABLE ROW LEVEL SECURITY;
CREATE POLICY items_admin ON catalog.items
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY items_empresa ON catalog.items
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.item_atributo_esquemas ENABLE ROW LEVEL SECURITY;
CREATE POLICY item_schemas_admin ON catalog.item_atributo_esquemas
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY item_schemas_lectura ON catalog.item_atributo_esquemas
  FOR SELECT TO authenticated
  USING (empresa_id IS NULL OR empresa_id::text = (auth.jwt() ->> 'tenant_id'));
CREATE POLICY item_schemas_empresa ON catalog.item_atributo_esquemas
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.proveedores ENABLE ROW LEVEL SECURITY;
CREATE POLICY proveedores_admin ON catalog.proveedores
  FOR ALL TO authenticated USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY proveedores_empresa ON catalog.proveedores
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.item_proveedores ENABLE ROW LEVEL SECURITY;
CREATE POLICY item_proveedores_admin ON catalog.item_proveedores
  FOR ALL TO authenticated USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY item_proveedores_empresa ON catalog.item_proveedores
  FOR ALL TO authenticated
  USING (EXISTS (SELECT 1 FROM catalog.items i
                 WHERE i.id = item_id AND i.empresa_id::text = (auth.jwt() ->> 'tenant_id')))
  WITH CHECK (EXISTS (SELECT 1 FROM catalog.items i
                     WHERE i.id = item_id AND i.empresa_id::text = (auth.jwt() ->> 'tenant_id')));

ALTER TABLE catalog.lotes_inventario ENABLE ROW LEVEL SECURITY;
CREATE POLICY lotes_admin ON catalog.lotes_inventario
  FOR ALL TO authenticated USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY lotes_empresa ON catalog.lotes_inventario
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

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


-- ── catalog.servicios (R-PS-02) ─────────────────────────────────────────
-- Los servicios son administrados por la empresa dueña; el catálogo público
-- de servicios no se expone todavía, así que no hay política anónima.
ALTER TABLE catalog.servicios ENABLE ROW LEVEL SECURITY;
CREATE POLICY servicios_admin ON catalog.servicios
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY servicios_empresa ON catalog.servicios
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

-- ── catalog: operaciones avanzadas (R-PS-04) ───────────────────────────
ALTER TABLE catalog.configuracion_inventario ENABLE ROW LEVEL SECURITY;
CREATE POLICY config_inventario_admin ON catalog.configuracion_inventario
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY config_inventario_empresa ON catalog.configuracion_inventario
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.combos ENABLE ROW LEVEL SECURITY;
CREATE POLICY combos_admin ON catalog.combos
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY combos_empresa ON catalog.combos
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.promociones ENABLE ROW LEVEL SECURITY;
CREATE POLICY promociones_admin ON catalog.promociones
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY promociones_empresa ON catalog.promociones
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

ALTER TABLE catalog.movimientos_inventario ENABLE ROW LEVEL SECURITY;
CREATE POLICY movimientos_admin ON catalog.movimientos_inventario
  FOR ALL TO authenticated
  USING (auth.rol() IN ('super_admin', 'admin'))
  WITH CHECK (auth.rol() IN ('super_admin', 'admin'));
CREATE POLICY movimientos_empresa ON catalog.movimientos_inventario
  FOR ALL TO authenticated
  USING (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'))
  WITH CHECK (auth.rol() = 'empresario' AND empresa_id::text = (auth.jwt() ->> 'tenant_id'));

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
