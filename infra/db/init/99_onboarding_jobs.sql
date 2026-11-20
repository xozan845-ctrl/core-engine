-- Onboarding idempotente de empresas (ADR-0047).
--
-- El alta de empresa ya es una transacción de identity; este job añade la
-- clave de idempotencia, el estado observable y la recuperación ante un
-- reintento tras un timeout. No orquesta servicios externos: el bucket de
-- storage sigue siendo lazy y las invitaciones usan su flujo propio.

BEGIN;

CREATE TABLE IF NOT EXISTS identity.provisioning_jobs (
  id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  actor_id        uuid NOT NULL REFERENCES identity.usuarios (id) ON DELETE CASCADE,
  idempotency_key text NOT NULL CHECK (char_length(idempotency_key) BETWEEN 8 AND 255),
  request_hash    text NOT NULL CHECK (char_length(request_hash) = 64),
  slug            text NOT NULL CHECK (char_length(slug) BETWEEN 2 AND 80),
  estado          text NOT NULL DEFAULT 'en_proceso'
                    CHECK (estado IN ('en_proceso', 'completado', 'fallido')),
  paso            text NOT NULL DEFAULT 'empresa'
                    CHECK (paso IN ('empresa', 'negocio', 'capacidades', 'completado')),
  empresa_id      uuid REFERENCES identity.empresas (id) ON DELETE SET NULL,
  negocio_id      uuid REFERENCES identity.negocios (id) ON DELETE SET NULL,
  resultado       jsonb NOT NULL DEFAULT '{}'::jsonb,
  error           text NOT NULL DEFAULT '',
  creado_en       timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en  timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (actor_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_provisioning_jobs_actor
  ON identity.provisioning_jobs (actor_id, creado_en DESC);

CREATE INDEX IF NOT EXISTS idx_provisioning_jobs_en_proceso
  ON identity.provisioning_jobs (actualizado_en)
  WHERE estado = 'en_proceso';

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.provisioning_jobs TO core_engine_app;
  END IF;
END $$;

COMMIT;
