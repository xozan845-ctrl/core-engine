-- ADR-0021 fase 4: negocio en envíos logísticos.
--
-- La guía se prepara por (orden, empresa, negocio). Los envíos existentes
-- quedan en el negocio principal de su empresa.
BEGIN;

ALTER TABLE logistics.envios
  ADD COLUMN IF NOT EXISTS negocio_id uuid;

UPDATE logistics.envios e
   SET negocio_id = n.id
  FROM identity.negocios n
 WHERE n.empresa_id = e.empresa_id
   AND n.es_predeterminado
   AND e.negocio_id IS NULL;

DROP INDEX IF EXISTS logistics.idx_envios_orden_empresa;
CREATE UNIQUE INDEX IF NOT EXISTS idx_envios_orden_empresa_negocio
  ON logistics.envios (order_id, empresa_id,
    COALESCE(negocio_id, '00000000-0000-0000-0000-000000000000'::uuid));

CREATE INDEX IF NOT EXISTS idx_envios_negocio
  ON logistics.envios (empresa_id, negocio_id, creado_en DESC);

COMMIT;
