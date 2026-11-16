-- ADR-0019 iteración 5: liquidaciones y compensaciones por negocio.
--
-- El corte quincenal consolida por (empresa, negocio) y mantiene el libro del
-- vendedor intacto. Las liquidaciones existentes quedan en el negocio principal.
BEGIN;

ALTER TABLE commissions.liquidaciones_empresas
  ADD COLUMN IF NOT EXISTS negocio_id uuid;

UPDATE commissions.liquidaciones_empresas l
   SET negocio_id = n.id
  FROM identity.negocios n
 WHERE n.empresa_id = l.empresa_id
   AND n.es_predeterminado
   AND l.negocio_id IS NULL;

ALTER TABLE commissions.compensaciones_devoluciones
  ADD COLUMN IF NOT EXISTS negocio_id uuid;

UPDATE commissions.compensaciones_devoluciones c
   SET negocio_id = COALESCE(
         (SELECT co.negocio_id FROM commissions.comisiones co WHERE co.id = c.comision_id),
         (SELECT n.id FROM identity.negocios n
           WHERE n.empresa_id = c.empresa_id AND n.es_predeterminado LIMIT 1)
       )
 WHERE c.negocio_id IS NULL;

ALTER TABLE commissions.liquidaciones_empresas
  DROP CONSTRAINT IF EXISTS liquidaciones_empresas_empresa_id_periodo_inicio_key;
CREATE UNIQUE INDEX IF NOT EXISTS idx_liquidaciones_empresa_negocio_periodo
  ON commissions.liquidaciones_empresas (empresa_id,
    COALESCE(negocio_id, '00000000-0000-0000-0000-000000000000'::uuid),
    periodo_inicio);

CREATE INDEX IF NOT EXISTS idx_liquidaciones_empresa_negocio
  ON commissions.liquidaciones_empresas (empresa_id, negocio_id, periodo_inicio DESC);
CREATE INDEX IF NOT EXISTS idx_compensaciones_negocio
  ON commissions.compensaciones_devoluciones (empresa_id, negocio_id, creado_en DESC);

COMMIT;
