-- Core Engine · identity — sesiones revocables (ADR-18, R-GW-4)
--
-- Cada login/registro emite una sesion identificada por el `jti` del refresh
-- token. El logout marca la sesion `revocada_en` (soft delete, R-DB-5) y el
-- `refresh` solo renueva mientras la sesion siga activa (no revocada y no
-- expirada). Idempotente (IF NOT EXISTS): seguro de aplicar sobre una BD ya
-- inicializada (R-DB-6).

CREATE TABLE IF NOT EXISTS identity.sesiones (
  id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  usuario_id  uuid NOT NULL REFERENCES identity.usuarios (id),
  jti         uuid NOT NULL UNIQUE,
  creada_en   timestamptz NOT NULL DEFAULT NOW(),
  expira_en   timestamptz NOT NULL,
  revocada_en timestamptz
);

-- R-DB-4: filtrado por usuario (listar/revocar sus sesiones).
CREATE INDEX IF NOT EXISTS idx_sesiones_usuario ON identity.sesiones (usuario_id);
