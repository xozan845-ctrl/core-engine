-- IVA configurable por tenant (ADR-0038).
--
-- `impuesto_activo` permite activar el desglose sin cambiar el comportamiento
-- histórico. `precios_con_impuesto` decide si el precio de catálogo ya incluye
-- IVA (habitual en retail) o si el impuesto se suma al total.
BEGIN;

ALTER TABLE finance.configuracion_fiscal
  ADD COLUMN IF NOT EXISTS impuesto_activo boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS precios_con_impuesto boolean NOT NULL DEFAULT true;

COMMIT;
