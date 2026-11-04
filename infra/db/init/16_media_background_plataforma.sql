-- Fondo del sitio de plataforma (ADR-38). El binario vive en storage-service;
-- identity solo guarda la referencia al objeto publico, el tipo derivado y los
-- metadatos de presentacion (sin FK cruzada, R-DB-1).
BEGIN;

CREATE TABLE IF NOT EXISTS identity.configuracion_plataforma (
  id smallint PRIMARY KEY DEFAULT 1 CHECK (id = 1),
  background_objeto_id uuid,
  background_media_type text CHECK (background_media_type IN ('imagen', 'video')),
  background_poster_objeto_id uuid,
  background_alt text NOT NULL DEFAULT '',
  background_overlay smallint NOT NULL DEFAULT 35
    CHECK (background_overlay BETWEEN 0 AND 100),
  background_position text NOT NULL DEFAULT 'center'
    CHECK (background_position IN ('center', 'top', 'bottom', 'left', 'right')),
  background_activo boolean NOT NULL DEFAULT false,
  actualizado_por uuid,
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);

INSERT INTO identity.configuracion_plataforma (id)
VALUES (1)
ON CONFLICT (id) DO NOTHING;

COMMIT;
