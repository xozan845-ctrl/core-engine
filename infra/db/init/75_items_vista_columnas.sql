-- Fase P2.5 · la vista publica de items recupera su contrato final.
-- `66_item_series.sql` y `68_retirar_combos.sql` añadieron `track_serial` y
-- `es_combo` a `catalog.items`, pero no recrearon `catalog.items_vista`
-- (definida por última vez en `48_catalog_retiro_legacy.sql`). Los clientes
-- que leen la vista recibían `column ... does not exist` y el dump de pruebas
-- del servicio ya declaraba ambas columnas: la migración era la que faltaba.
BEGIN;

DROP VIEW IF EXISTS catalog.items_vista;

CREATE VIEW catalog.items_vista AS
SELECT i.id AS item_id,
       i.empresa_id,
       i.tipo,
       i.codigo,
       i.nombre,
       i.descripcion,
       i.categoria,
       i.precio_cents,
       i.costo_cents,
       i.track_inventory,
       i.stock,
       i.duration_minutes,
       i.atributos,
       i.estado,
       i.parent_item_id,
       i.marca,
       i.codigo_barras,
       i.unidad,
       i.permite_fracciones,
       i.unidad_compra,
       i.contenido_cantidad,
       i.contenido_unidad,
       i.tipo_precio,
       i.tipo_costo,
       i.stock_minimo,
       i.stock_maximo,
       i.garantia_meses,
       i.es_materia_prima,
       i.ubicacion,
       i.track_serial,
       i.es_combo,
       i.creado_en,
       i.actualizado_en
  FROM catalog.items i
 WHERE i.eliminado_en IS NULL;

-- La vista se recrea después de `51_app_grants.sql`; se reotorga el acceso a
-- la cuenta de aplicación para no depender de privilegios por defecto.
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT ON catalog.items_vista TO core_engine_app;
  END IF;
END $$;

COMMENT ON VIEW catalog.items_vista IS
  'Items activos con el contrato publico: incluye serializacion y combos.';

COMMIT;
