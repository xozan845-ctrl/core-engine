-- RLS: historial de precios de ofertas.
--
-- Lo escribe el vendedor dueño de la oferta al publicar o editar precios (y el
-- servicio/admin). Sin esta política la tabla era admin-only y publicar una
-- oferta devolvía 500 bajo RLS endurecido.
BEGIN;

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

COMMIT;
