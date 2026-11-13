-- Limpieza · unificar catalog.combos con el modelo items + BOM.
-- Los combos pasan a ser items con es_combo=true y componentes en
-- catalog.item_componentes. Se conserva el id para no romper clientes.
BEGIN;

ALTER TABLE catalog.items
  ADD COLUMN IF NOT EXISTS es_combo boolean NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_items_es_combo
  ON catalog.items (empresa_id, creado_en DESC)
  WHERE es_combo AND eliminado_en IS NULL;

INSERT INTO catalog.items
  (id, empresa_id, tipo, codigo, nombre, descripcion, categoria, precio_cents,
   costo_cents, track_inventory, stock, atributos, estado, codigo_barras,
   es_combo, creado_en, actualizado_en)
SELECT c.id, c.empresa_id, 'producto', c.sku, c.nombre, c.descripcion, 'combo',
       c.precio_cents, 0, false, NULL, '{}'::jsonb, c.estado, c.codigo_barras,
       true, c.creado_en, c.actualizado_en
  FROM catalog.combos c
 WHERE c.eliminado_en IS NULL
ON CONFLICT DO NOTHING;

WITH componentes AS (
  SELECT c.empresa_id, c.id AS combo_id, elem
    FROM catalog.combos c,
         LATERAL jsonb_array_elements(c.componentes) AS elem
   WHERE c.eliminado_en IS NULL
     AND elem ? 'item_id'
     AND (elem->>'item_id') ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
     AND COALESCE((elem->>'cantidad')::int, 0) > 0
)
INSERT INTO catalog.item_componentes
  (empresa_id, item_id, componente_item_id, cantidad)
SELECT componentes.empresa_id,
       componentes.combo_id,
       (componentes.elem->>'item_id')::uuid,
       (componentes.elem->>'cantidad')::int
  FROM componentes
  JOIN catalog.items padre
    ON padre.id = componentes.combo_id AND padre.es_combo
  JOIN catalog.items componente
    ON componente.id = (componentes.elem->>'item_id')::uuid
   AND componente.empresa_id = componentes.empresa_id
   AND componente.tipo = 'producto'
   AND componente.eliminado_en IS NULL
ON CONFLICT (item_id, componente_item_id) DO NOTHING;

DROP TABLE IF EXISTS catalog.combos CASCADE;

COMMIT;
