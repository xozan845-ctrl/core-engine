-- Reset de contraseña de identity (ADR-0003, fase 2).
--
-- Solo se persiste el hash SHA-256 del token de un solo uso; el token en claro
-- viaja únicamente en el correo. Al solicitar uno nuevo se invalidan los
-- anteriores del mismo usuario. Idempotente (IF NOT EXISTS) para bases ya
-- inicializadas (R-DB-6).

BEGIN;

CREATE TABLE IF NOT EXISTS identity.password_reset_tokens (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  usuario_id  uuid NOT NULL REFERENCES identity.usuarios (id) ON DELETE CASCADE,
  token_hash  text NOT NULL UNIQUE,
  expira_en   timestamptz NOT NULL,
  usado_en    timestamptz,
  creado_en   timestamptz NOT NULL DEFAULT NOW()
);

-- R-DB-4: búsqueda de los tokens vigentes de un usuario (invalidación previa).
CREATE INDEX IF NOT EXISTS idx_password_reset_usuario
  ON identity.password_reset_tokens (usuario_id, creado_en DESC);

COMMIT;
