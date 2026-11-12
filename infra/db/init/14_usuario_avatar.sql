-- Avatar de perfil de usuario/vendedor. El binario vive en storage-service;
-- identity solo guarda la referencia al objeto público y lo valida por API
-- interna (sin FK cruzada, R-DB-1).
BEGIN;

ALTER TABLE identity.usuarios
  ADD COLUMN IF NOT EXISTS avatar_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS actualizado_en timestamptz NOT NULL DEFAULT NOW();

COMMIT;
