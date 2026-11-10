-- Fase 3 · catalog.items como única fuente del catálogo (R-DB-6).
-- Aditivo: completa los metadatos que vivían en productos/servicios, migra
-- variantes e imágenes y mueve las referencias de operaciones a item_id.
-- La retirada de las tablas legacy ocurre en 45_catalog_retiro_legacy.sql.
BEGIN;

-- ── metadatos operativos en items ────────────────────────────────────────
ALTER TABLE catalog.items
  ADD COLUMN IF NOT EXISTS marca text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS codigo_barras text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS unidad text NOT NULL DEFAULT 'Unidad (u)',
  ADD COLUMN IF NOT EXISTS permite_fracciones boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS unidad_compra text NOT NULL DEFAULT 'Igual que la de venta',
  ADD COLUMN IF NOT EXISTS contenido_cantidad integer NOT NULL DEFAULT 0
    CHECK (contenido_cantidad >= 0),
  ADD COLUMN IF NOT EXISTS contenido_unidad text NOT NULL DEFAULT 'Sin contenido',
  ADD COLUMN IF NOT EXISTS tipo_precio text NOT NULL DEFAULT 'fijo'
    CHECK (tipo_precio IN ('fijo', 'escala')),
  ADD COLUMN IF NOT EXISTS tipo_costo text NOT NULL DEFAULT 'fijo'
    CHECK (tipo_costo IN ('fijo', 'escala')),
  ADD COLUMN IF NOT EXISTS stock_minimo integer NOT NULL DEFAULT 0
    CHECK (stock_minimo >= 0),
  ADD COLUMN IF NOT EXISTS stock_maximo integer
    CHECK (stock_maximo IS NULL OR stock_maximo >= 0),
  ADD COLUMN IF NOT EXISTS garantia_meses integer
    CHECK (garantia_meses IS NULL OR garantia_meses >= 0),
  ADD COLUMN IF NOT EXISTS es_materia_prima boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS ubicacion text NOT NULL DEFAULT '';

-- Backfill de metadatos desde las fuentes legacy mientras siguen existiendo.
UPDATE catalog.items i
   SET costo_cents = c.costo_cents,
       marca = c.marca,
       codigo_barras = c.codigo_barras,
       unidad = c.unidad,
       permite_fracciones = c.permite_fracciones,
       unidad_compra = c.unidad_compra,
       contenido_cantidad = c.contenido_cantidad,
       contenido_unidad = c.contenido_unidad,
       tipo_precio = c.tipo_precio,
       tipo_costo = c.tipo_costo,
       stock_minimo = c.stock_minimo,
       stock_maximo = c.stock_maximo,
       garantia_meses = c.garantia_meses,
       es_materia_prima = c.es_materia_prima,
       ubicacion = c.ubicacion,
       track_inventory = true,
       actualizado_en = NOW()
  FROM catalog.productos c
 WHERE i.source_type = 'producto' AND i.source_id = c.id;

UPDATE catalog.items i
   SET marca = s.marca,
       codigo_barras = s.codigo_barras,
       unidad = s.unidad,
       actualizado_en = NOW()
  FROM catalog.servicios s
 WHERE i.source_type = 'servicio' AND i.source_id = s.id;

-- Variantes: el hijo apunta a la fila de items que representa al padre.
UPDATE catalog.items hijo
   SET parent_item_id = padre.id
  FROM catalog.productos c
  JOIN catalog.items padre
    ON padre.source_type = 'producto' AND padre.source_id = c.producto_padre_id
 WHERE hijo.source_type = 'producto'
   AND hijo.source_id = c.id
   AND c.producto_padre_id IS NOT NULL
   AND (hijo.parent_item_id IS NULL OR hijo.parent_item_id <> padre.id);

-- Imágenes: si 43 ya hizo el backfill esto es idempotente.
INSERT INTO catalog.item_imagenes (id, empresa_id, item_id, objeto_id, orden, es_principal)
SELECT pi.id, p.empresa_id, i.id, pi.objeto_id, pi.orden, pi.es_principal
  FROM catalog.producto_imagenes pi
  JOIN catalog.productos p ON p.id = pi.producto_id
  JOIN catalog.items i ON i.source_type = 'producto' AND i.source_id = p.id
ON CONFLICT DO NOTHING;

-- El código de barras pasa a ser único por empresa en todo el catálogo.
CREATE UNIQUE INDEX IF NOT EXISTS ux_items_empresa_barcode
  ON catalog.items (empresa_id, codigo_barras)
  WHERE codigo_barras <> '' AND eliminado_en IS NULL;

-- ── operaciones: referencias por item_id ─────────────────────────────────
ALTER TABLE catalog.promociones
  ADD COLUMN IF NOT EXISTS item_id uuid REFERENCES catalog.items(id) ON DELETE SET NULL;
UPDATE catalog.promociones p
   SET item_id = i.id
  FROM catalog.items i
 WHERE p.producto_id IS NOT NULL
   AND i.source_type = 'producto' AND i.source_id = p.producto_id
   AND p.item_id IS NULL;

ALTER TABLE catalog.historico_precios
  ADD COLUMN IF NOT EXISTS item_id uuid REFERENCES catalog.items(id) ON DELETE CASCADE;
UPDATE catalog.historico_precios h
   SET item_id = i.id
  FROM catalog.items i
 WHERE h.item_id IS NULL
   AND i.empresa_id = h.empresa_id
   AND LOWER(i.codigo) = LOWER(h.producto_sku);

ALTER TABLE catalog.ajustes_stock
  ADD COLUMN IF NOT EXISTS item_id uuid REFERENCES catalog.items(id) ON DELETE CASCADE;
UPDATE catalog.ajustes_stock a
   SET item_id = i.id
  FROM catalog.items i
 WHERE a.item_id IS NULL
   AND i.empresa_id = a.empresa_id
   AND LOWER(i.codigo) = LOWER(a.producto_sku);

-- Combos: componentes pasan de `producto_id` a `item_id`.
UPDATE catalog.combos c
   SET componentes = sub.componentes
  FROM (
    SELECT c2.id,
           jsonb_agg(
             jsonb_build_object(
               'item_id', COALESCE(i.id, (elem->>'producto_id')::uuid),
               'sku', elem->>'sku',
               'nombre', elem->>'nombre',
               'cantidad', (elem->>'cantidad')::int
             )
             ORDER BY elem->>'sku'
           ) AS componentes
      FROM catalog.combos c2,
           jsonb_array_elements(c2.componentes) AS elem
      LEFT JOIN catalog.items i
        ON i.source_type = 'producto'
       AND i.source_id = (elem->>'producto_id')::uuid
     WHERE elem ? 'producto_id'
     GROUP BY c2.id
  ) sub
 WHERE c.id = sub.id;

-- ── vista de lectura con metadatos ───────────────────────────────────────
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
       i.source_type,
       i.source_id,
       i.creado_en,
       i.actualizado_en
  FROM catalog.items i
 WHERE i.eliminado_en IS NULL;

COMMIT;
