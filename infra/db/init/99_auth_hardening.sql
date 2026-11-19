-- Endurecimiento de autenticación (ADR-0036).
--
-- Añade verificación de correo, lockout por cuenta, MFA TOTP con códigos de
-- recuperación y metadatos de sesión/dispositivo. Aditivo e idempotente.
BEGIN;

ALTER TABLE identity.usuarios
  ADD COLUMN IF NOT EXISTS correo_verificado_en timestamptz,
  ADD COLUMN IF NOT EXISTS intentos_fallidos integer NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS bloqueado_hasta timestamptz,
  ADD COLUMN IF NOT EXISTS mfa_habilitado boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS mfa_secreto text,
  ADD COLUMN IF NOT EXISTS password_actualizado_en timestamptz,
  ADD COLUMN IF NOT EXISTS ultimo_login_en timestamptz;

-- Los usuarios existentes se consideran verificados: la exigencia aplica a
-- registros nuevos y no debe bloquear cuentas ya operativas.
UPDATE identity.usuarios
   SET correo_verificado_en = COALESCE(correo_verificado_en, creado_en)
 WHERE correo_verificado_en IS NULL;

ALTER TABLE identity.sesiones
  ADD COLUMN IF NOT EXISTS user_agent text,
  ADD COLUMN IF NOT EXISTS ip text,
  ADD COLUMN IF NOT EXISTS ultimo_uso_en timestamptz;

CREATE TABLE IF NOT EXISTS identity.correo_verificacion_tokens (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  usuario_id  uuid NOT NULL REFERENCES identity.usuarios(id) ON DELETE CASCADE,
  token_hash  text NOT NULL,
  expira_en   timestamptz NOT NULL,
  usado_en    timestamptz,
  creado_en   timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_correo_verificacion_token
  ON identity.correo_verificacion_tokens (token_hash);
CREATE INDEX IF NOT EXISTS idx_correo_verificacion_usuario
  ON identity.correo_verificacion_tokens (usuario_id, creado_en DESC);

CREATE TABLE IF NOT EXISTS identity.mfa_codigos_recuperacion (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  usuario_id  uuid NOT NULL REFERENCES identity.usuarios(id) ON DELETE CASCADE,
  codigo_hash text NOT NULL,
  usado_en    timestamptz,
  creado_en   timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_mfa_codigos_usuario
  ON identity.mfa_codigos_recuperacion (usuario_id, usado_en);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE
      ON identity.correo_verificacion_tokens TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE
      ON identity.mfa_codigos_recuperacion TO core_engine_app;
  END IF;
END $$;

COMMIT;
