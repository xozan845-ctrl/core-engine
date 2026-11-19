-- Exportación asíncrona y firmada del tenant (ADR-0042).
BEGIN;

CREATE TABLE IF NOT EXISTS identity.exportacion_jobs (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  estado         text NOT NULL DEFAULT 'pendiente'
                 CHECK (estado IN ('pendiente', 'en_proceso', 'completado', 'fallido')),
  formato        text NOT NULL DEFAULT 'core-engine.tenant-export.v2',
  solicitado_por uuid REFERENCES identity.usuarios(id) ON DELETE SET NULL,
  contenido      bytea,
  tamano_bytes   bigint,
  sha256         text,
  firma          text,
  servicios      jsonb NOT NULL DEFAULT '[]'::jsonb,
  error          text,
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  iniciado_en    timestamptz,
  completado_en  timestamptz
);

CREATE INDEX IF NOT EXISTS idx_exportacion_jobs_empresa
  ON identity.exportacion_jobs (empresa_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_exportacion_jobs_estado
  ON identity.exportacion_jobs (estado, creado_en);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE
      ON identity.exportacion_jobs TO core_engine_app;
  END IF;
END $$;

COMMIT;
