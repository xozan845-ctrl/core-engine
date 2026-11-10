-- Fase 3 · modelo de items con variantes e imágenes.
-- Aditivo: agrega parent_item_id e item_imagenes y hace backfill desde las
-- tablas legacy. No retira todavía productos/servicios.
BEGIN;

ALTER TABLE catalog.items
  ADD COLUMN IF NOT EXISTS parent_item_id uuid REFERENCES catalog.items(id);
CREATE INDEX IF NOT EXISTS idx_items_parent
  ON catalog.items (parent_item_id, estado)
  WHERE eliminado_en IS NULL;

CREATE TABLE IF NOT EXISTS catalog.item_imagenes (
  id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id   uuid NOT NULL,
  item_id      uuid NOT NULL REFERENCES catalog.items(id) ON DELETE CASCADE,
  objeto_id    uuid NOT NULL,
  orden        smallint NOT NULL CHECK (orden BETWEEN 0 AND 2),
  es_principal boolean NOT NULL DEFAULT false,
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (item_id, objeto_id),
  UNIQUE (item_id, orden)
);
CREATE INDEX IF NOT EXISTS idx_item_imagenes_empresa
  ON catalog.item_imagenes (empresa_id, item_id);
CREATE UNIQUE INDEX IF NOT EXISTS ux_item_imagen_principal
  ON catalog.item_imagenes (item_id) WHERE es_principal;

CREATE OR REPLACE VIEW catalog.items_vista AS
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
       i.source_type,
       i.source_id,
       i.creado_en,
       i.actualizado_en
  FROM catalog.items i
 WHERE i.eliminado_en IS NULL;

-- Backfill de variantes: el hijo legacy apunta al item padre.
UPDATE catalog.items hijo
   SET parent_item_id = padre.id
  FROM catalog.productos c
  JOIN catalog.items padre
    ON padre.source_type = 'producto' AND padre.source_id = c.producto_padre_id
 WHERE hijo.source_type = 'producto'
   AND hijo.source_id = c.id
   AND c.producto_padre_id IS NOT NULL
   AND hijo.parent_item_id IS NULL;

-- Backfill de imágenes.
INSERT INTO catalog.item_imagenes (id, empresa_id, item_id, objeto_id, orden, es_principal)
SELECT pi.id, p.empresa_id, i.id, pi.objeto_id, pi.orden, pi.es_principal
  FROM catalog.producto_imagenes pi
  JOIN catalog.productos p ON p.id = pi.producto_id
  JOIN catalog.items i ON i.source_type = 'producto' AND i.source_id = p.id
ON CONFLICT DO NOTHING;

COMMIT;
