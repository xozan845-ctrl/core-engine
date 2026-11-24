-- PDF de comprobantes en object storage (ADR-0042).
-- `contenido` queda opcional: si storage responde, el PDF vive como objeto.
BEGIN;

ALTER TABLE finance.comprobante_pdf
  ALTER COLUMN contenido DROP NOT NULL,
  ADD COLUMN IF NOT EXISTS objeto_id uuid;

CREATE INDEX IF NOT EXISTS idx_comprobante_pdf_objeto
  ON finance.comprobante_pdf (objeto_id)
  WHERE objeto_id IS NOT NULL;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON finance.comprobante_pdf TO core_engine_app;
  END IF;
END $$;

COMMIT;
