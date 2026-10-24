-- ADR-27: la demanda de un SKU deja de ser global.
-- Migración aditiva: `intelligence.metricas_producto` pasa a clave
-- (empresa_id, sku) para no mezclar empresas con el mismo SKU. Se ejecuta
-- después de 03_intelligence.sql, que crea el esquema y la tabla.
BEGIN;

ALTER TABLE intelligence.metricas_producto ADD COLUMN IF NOT EXISTS empresa_id uuid;

-- Backfill: primero por producto real, luego el resto al catálogo legado.
UPDATE intelligence.metricas_producto m
   SET empresa_id = p.empresa_id
  FROM catalog.productos p
 WHERE LOWER(p.sku) = LOWER(m.sku)
   AND m.empresa_id IS NULL;
UPDATE intelligence.metricas_producto
   SET empresa_id = '00000000-0000-0000-0000-000000000000'
 WHERE empresa_id IS NULL;

ALTER TABLE intelligence.metricas_producto ALTER COLUMN empresa_id SET NOT NULL;
ALTER TABLE intelligence.metricas_producto DROP CONSTRAINT IF EXISTS metricas_producto_pkey;
ALTER TABLE intelligence.metricas_producto
  ADD CONSTRAINT metricas_producto_pkey PRIMARY KEY (empresa_id, sku);
CREATE INDEX IF NOT EXISTS idx_metricas_producto_sku
  ON intelligence.metricas_producto (sku);

COMMIT;
