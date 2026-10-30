-- Metadatos de empresa (R-PS-01 aplicado a identidad): perfil comercial,
-- datos fiscales opcionales y logo. El logo se sube por storage-service y aquí
-- solo se referencia el objeto (sin FK cruzada entre servicios, R-DB-1).
BEGIN;

ALTER TABLE identity.empresas
  ADD COLUMN IF NOT EXISTS ruc text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS razon_social text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS nombre_comercial text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS descripcion text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS telefono text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS correo_contacto text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS sitio_web text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS direccion text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS ciudad text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS departamento text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS pais text NOT NULL DEFAULT 'Nicaragua',
  ADD COLUMN IF NOT EXISTS moneda text NOT NULL DEFAULT 'NIO',
  ADD COLUMN IF NOT EXISTS zona_horaria text NOT NULL DEFAULT 'America/Managua',
  ADD COLUMN IF NOT EXISTS sector text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS representante_legal text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS horario_atencion text NOT NULL DEFAULT '',
  ADD COLUMN IF NOT EXISTS logo_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS actualizado_en timestamptz NOT NULL DEFAULT NOW();

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_constraint WHERE conname = 'empresas_moneda_check'
  ) THEN
    ALTER TABLE identity.empresas
      ADD CONSTRAINT empresas_moneda_check CHECK (char_length(moneda) = 3);
  END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS ux_empresas_ruc
  ON identity.empresas (upper(ruc)) WHERE ruc <> '';

COMMIT;
