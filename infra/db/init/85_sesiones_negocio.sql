-- ADR-0019 iteración 6: negocio activo por sesión.
--
-- El negocio activo deja de depender de la asignación más reciente: cada
-- refresh token conserva el negocio elegido y el cambio se hace por sesión.
BEGIN;

ALTER TABLE identity.sesiones
  ADD COLUMN IF NOT EXISTS negocio_id uuid;

CREATE INDEX IF NOT EXISTS idx_sesiones_negocio
  ON identity.sesiones (usuario_id, negocio_id)
  WHERE revocada_en IS NULL;

COMMIT;
