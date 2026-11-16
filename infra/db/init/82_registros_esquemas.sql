-- Fase 5 · esquemas JSON versionados por tipo de registro.
-- Permiten validar historia clínica, recetas, expedientes, mantenimiento, etc.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.registros_esquemas (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL REFERENCES identity.empresas(id) ON DELETE CASCADE,
  tipo           text NOT NULL CHECK (tipo ~ '^[a-z][a-z0-9_]{1,39}$'),
  version        integer NOT NULL CHECK (version >= 1),
  esquema        jsonb NOT NULL CHECK (jsonb_typeof(esquema) = 'object'),
  activo         boolean NOT NULL DEFAULT true,
  creado_por     uuid NOT NULL REFERENCES identity.usuarios(id),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW(),
  UNIQUE (empresa_id, tipo, version)
);

CREATE UNIQUE INDEX IF NOT EXISTS ux_registros_esquemas_activo
  ON identity.registros_esquemas (empresa_id, tipo)
  WHERE activo;

COMMIT;
