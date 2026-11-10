-- Fase 4: partes como única referencia de cliente.
-- Se descartan los IDs legacy que no tengan equivalente en identity.partes
-- (no hay datos productivos que preservar).
BEGIN;

-- Backfill donde existe un mapeo claro vía field.clientes.
UPDATE field.pedidos p
   SET parte_id = c.parte_id
  FROM field.clientes c
 WHERE p.cliente_id = c.id AND p.parte_id IS NULL;

UPDATE field.paradas p
   SET parte_id = c.parte_id
  FROM field.clientes c
 WHERE p.cliente_id = c.id AND p.parte_id IS NULL;

UPDATE field.visitas v
   SET parte_id = c.parte_id
  FROM field.clientes c
 WHERE v.cliente_id = c.id AND v.parte_id IS NULL;

ALTER TABLE field.pedidos DROP COLUMN IF EXISTS cliente_id;
ALTER TABLE field.paradas DROP COLUMN IF EXISTS cliente_id;
ALTER TABLE field.visitas DROP COLUMN IF EXISTS cliente_id;

-- Citas y comprobantes: el valor legacy no se puede mapear a partes.
ALTER TABLE scheduling.citas DROP COLUMN IF EXISTS cliente_id;
ALTER TABLE finance.comprobantes DROP COLUMN IF EXISTS cliente_id;

COMMIT;
