-- API keys de clientes máquina (ADR-0005, R-GW-8).
--
-- Solo se persiste el hash SHA-256 de la clave; el secreto se muestra una vez
-- al crearla. La clave se intercambia por un JWT corto en /api/v2/auth/token.
-- Idempotente (IF NOT EXISTS) para bases ya inicializadas (R-DB-6).

BEGIN;

CREATE TABLE IF NOT EXISTS identity.api_keys (
  id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id   uuid NOT NULL REFERENCES identity.empresas (id) ON DELETE CASCADE,
  nombre       text NOT NULL CHECK (char_length(trim(nombre)) BETWEEN 2 AND 120),
  prefijo      text NOT NULL CHECK (char_length(prefijo) BETWEEN 4 AND 16),
  hash         text NOT NULL UNIQUE CHECK (char_length(hash) = 64),
  rol          text NOT NULL CHECK (rol IN (
                 'empresario', 'vendedor', 'logistica',
                 'coordinador', 'supervisor', 'operativo'
               )),
  estado       text NOT NULL DEFAULT 'activa' CHECK (estado IN ('activa', 'revocada')),
  expira_en    timestamptz,
  creado_por   uuid REFERENCES identity.usuarios (id) ON DELETE SET NULL,
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  ultimo_uso_en timestamptz
);

CREATE INDEX IF NOT EXISTS idx_api_keys_empresa_estado
  ON identity.api_keys (empresa_id, estado, creado_en DESC);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.api_keys TO core_engine_app;
  END IF;
END $$;

COMMIT;
