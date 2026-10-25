-- ADR-28: imágenes públicas servidas por storage-service.
-- `publico` es una capacidad explícita: solo los objetos marcados así se
-- sirven por el endpoint estático sin autenticación.
BEGIN;

ALTER TABLE storage.objetos ADD COLUMN IF NOT EXISTS publico boolean NOT NULL DEFAULT false;

CREATE INDEX IF NOT EXISTS idx_storage_objetos_publicos
  ON storage.objetos (publico, estado)
  WHERE publico;

COMMIT;
