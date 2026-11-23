-- Amplía el catálogo de tipos de proveedor de pago antes de sembrar las
-- pasarelas concretas (99z_pasarelas.sql).
--
-- El constraint original de 99_pagos_configurables solo permitía
-- simulado/http/manual. Este archivo corre antes por orden lexicográfico
-- (99y < 99z) y es idempotente.
BEGIN;

ALTER TABLE orders.proveedores_pago
  DROP CONSTRAINT IF EXISTS proveedores_pago_tipo_check;
ALTER TABLE orders.proveedores_pago
  ADD CONSTRAINT proveedores_pago_tipo_check
  CHECK (tipo IN ('simulado', 'http', 'manual', 'tilopay', 'pagadito'));

COMMIT;
