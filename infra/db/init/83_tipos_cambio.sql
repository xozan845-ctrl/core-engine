-- Fase 7 · tipos de cambio publicados (base → quote).
-- La tasa vigente es la última por `vigente_desde`; no hay conversión
-- implícita: orders solo convierte si el cliente pide `moneda_pago`.
BEGIN;

CREATE TABLE IF NOT EXISTS finance.tipos_cambio (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  base           text NOT NULL CHECK (base ~ '^[A-Z]{3}$'),
  quote          text NOT NULL CHECK (quote ~ '^[A-Z]{3}$'),
  tasa           numeric(18,8) NOT NULL CHECK (tasa > 0),
  vigente_desde  timestamptz NOT NULL DEFAULT NOW(),
  creado_por     uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  CHECK (base <> quote)
);

CREATE INDEX IF NOT EXISTS idx_tipos_cambio_vigente
  ON finance.tipos_cambio (base, quote, vigente_desde DESC);

COMMIT;
