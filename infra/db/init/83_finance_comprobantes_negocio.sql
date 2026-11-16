-- ADR-0019: comprobantes fiscales por negocio.
--
-- El comprobante conserva la orden como referencia; `negocio_id` permite
-- reportes de IVA/ventas por unidad operativa.
BEGIN;

ALTER TABLE finance.comprobantes
  ADD COLUMN IF NOT EXISTS negocio_id uuid;

UPDATE finance.comprobantes c
   SET negocio_id = o.negocio_id
  FROM orders.orden_vista o
 WHERE o.id = c.orden_id
   AND c.negocio_id IS NULL
   AND o.negocio_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_comprobantes_negocio
  ON finance.comprobantes (negocio_id, creado_en DESC)
  WHERE negocio_id IS NOT NULL;

COMMIT;
