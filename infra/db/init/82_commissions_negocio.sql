-- ADR-0019: comisiones por negocio.
--
-- Las comisiones ya se guardaban por empresa; `negocio_id` permite reportes y
-- liquidaciones por unidad operativa sin cambiar el cálculo de RN-04.
BEGIN;

ALTER TABLE commissions.comisiones
  ADD COLUMN IF NOT EXISTS negocio_id uuid;

UPDATE commissions.comisiones c
   SET negocio_id = n.id
  FROM identity.negocios n
 WHERE n.empresa_id = c.empresa_id
   AND n.es_predeterminado
   AND c.negocio_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_comisiones_negocio
  ON commissions.comisiones (empresa_id, negocio_id, creado_en DESC);

COMMIT;
