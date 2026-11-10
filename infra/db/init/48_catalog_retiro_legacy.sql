-- Fase 3 · retirada del catálogo legacy (R-DB-6).
-- Elimina las tablas/trigger de sincronización productos/servicios→items y las
-- referencias que solo existían para mantenerlas. items es la única fuente.
BEGIN;

-- ── triggers y funciones de sincronización/proyección ────────────────────
DROP TRIGGER IF EXISTS trg_sync_item_producto ON catalog.productos;
DROP TRIGGER IF EXISTS trg_sync_item_servicio ON catalog.servicios;
DROP TRIGGER IF EXISTS productos_proyeccion_trigger ON catalog.productos;
DROP TRIGGER IF EXISTS reservas_proyeccion_trigger ON catalog.reservas_ordenes;

DROP FUNCTION IF EXISTS catalog.fn_sync_item_producto() CASCADE;
DROP FUNCTION IF EXISTS catalog.fn_sync_item_servicio() CASCADE;
DROP FUNCTION IF EXISTS catalog.fn_trigger_producto() CASCADE;
DROP FUNCTION IF EXISTS catalog.fn_trigger_reserva() CASCADE;
DROP FUNCTION IF EXISTS catalog.fn_bootstrap_proyecciones() CASCADE;
DROP FUNCTION IF EXISTS catalog.fn_recalcular_padre(uuid) CASCADE;
DROP FUNCTION IF EXISTS catalog.fn_refrescar_mes_inventario(date) CASCADE;

-- ── referencias externas al producto legacy ──────────────────────────────
ALTER TABLE catalog.historico_precios DROP COLUMN IF EXISTS producto_sku CASCADE;
ALTER TABLE catalog.ajustes_stock DROP COLUMN IF EXISTS producto_sku CASCADE;
ALTER TABLE catalog.promociones DROP COLUMN IF EXISTS producto_id CASCADE;
ALTER TABLE stores.ofertas DROP COLUMN IF EXISTS producto_id CASCADE;
ALTER TABLE stores.ofertas DROP COLUMN IF EXISTS producto_padre_id CASCADE;

-- ── tablas y proyecciones legacy ─────────────────────────────────────────
DROP TABLE IF EXISTS catalog.producto_imagenes CASCADE;
DROP TABLE IF EXISTS catalog.productos CASCADE;
DROP TABLE IF EXISTS catalog.servicios CASCADE;
DROP TABLE IF EXISTS catalog.producto_catalogo_vista CASCADE;
DROP TABLE IF EXISTS catalog.stock_vista CASCADE;
DROP TABLE IF EXISTS catalog.inventario_mensual_mv CASCADE;

-- ── items sin columnas de origen ─────────────────────────────────────────
DROP VIEW IF EXISTS catalog.items_vista;
ALTER TABLE catalog.items DROP COLUMN IF EXISTS source_type;
ALTER TABLE catalog.items DROP COLUMN IF EXISTS source_id;

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
       i.creado_en,
       i.actualizado_en
  FROM catalog.items i
 WHERE i.eliminado_en IS NULL;

COMMIT;
