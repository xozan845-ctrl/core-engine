-- Cobro centralizado en la plataforma (ADR-0053).
--
-- La plataforma es el merchant of record: el consumidor paga a la cuenta de la
-- plataforma y esta liquida al negocio y al vendedor. La pasarela puede
-- informar en su respuesta/consulta la comisión, impuestos, retenciones y el
-- monto neto liquidado; se persisten para el ledger de dispersión diaria.
BEGIN;

ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS proveedor_liquidacion text NOT NULL DEFAULT '';
ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS comision_pasarela_cents integer NOT NULL DEFAULT 0;
ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS impuesto_pasarela_cents integer NOT NULL DEFAULT 0;
ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS retencion_cents integer NOT NULL DEFAULT 0;
ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS monto_liquidado_cents integer NOT NULL DEFAULT 0;
ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS moneda_liquidacion text NOT NULL DEFAULT '';
ALTER TABLE orders.pagos
  ADD COLUMN IF NOT EXISTS detalle_pasarela jsonb NOT NULL DEFAULT '{}'::jsonb;

COMMIT;
