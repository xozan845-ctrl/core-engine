-- Almacenamiento del PDF generado por comprobante (ADR-0040).
BEGIN;

CREATE TABLE IF NOT EXISTS finance.comprobante_pdf (
  comprobante_id uuid PRIMARY KEY REFERENCES finance.comprobantes(id) ON DELETE CASCADE,
  contenido      bytea NOT NULL,
  generado_en    timestamptz NOT NULL DEFAULT NOW()
);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON finance.comprobante_pdf TO core_engine_app;
  END IF;
END $$;

COMMIT;
