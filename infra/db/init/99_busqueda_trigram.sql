-- Búsqueda por similitud en catálogo y clientes (ADR-0043).
-- Los listados usan `LIKE '%q%'`/`ILIKE`; sin trigram el plan es un scan.
BEGIN;

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS idx_item_items_nombre_trgm
  ON item.items USING gin (lower(nombre) gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_item_items_codigo_trgm
  ON item.items USING gin (lower(codigo) gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_item_items_codigo_barras_trgm
  ON item.items USING gin (lower(codigo_barras) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_partes_nombre_trgm
  ON identity.partes USING gin (nombre gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_partes_nombre_comercial_trgm
  ON identity.partes USING gin (nombre_comercial gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_partes_identificador_trgm
  ON identity.partes USING gin (identificador_valor gin_trgm_ops);

COMMIT;
