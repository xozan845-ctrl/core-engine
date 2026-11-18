-- Reseñas de comprador y wishlist (favoritos).
--
-- Una reseña por comprador e item. La moderación es explícita: nace
-- `pendiente` y solo se publica cuando un admin la aprueba. La compra
-- verificada queda fuera de esta fase (no se consulta orders todavía).
BEGIN;

CREATE TABLE IF NOT EXISTS stores.resenas (
  id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
  empresa_id     uuid NOT NULL,
  item_id        uuid NOT NULL,
  comprador_id   uuid NOT NULL,
  calificacion   integer NOT NULL CHECK (calificacion BETWEEN 1 AND 5),
  titulo         text NOT NULL DEFAULT '' CHECK (char_length(titulo) <= 120),
  comentario     text NOT NULL DEFAULT '' CHECK (char_length(comentario) <= 2000),
  estado         text NOT NULL DEFAULT 'pendiente'
                 CHECK (estado IN ('pendiente', 'publicada', 'rechazada')),
  creado_en      timestamptz NOT NULL DEFAULT NOW(),
  actualizado_en timestamptz NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS ux_resenas_item_comprador
  ON stores.resenas (item_id, comprador_id);
CREATE INDEX IF NOT EXISTS idx_resenas_item_estado
  ON stores.resenas (item_id, estado, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_resenas_comprador
  ON stores.resenas (comprador_id, creado_en DESC);

CREATE TABLE IF NOT EXISTS stores.favoritos (
  comprador_id uuid NOT NULL,
  item_id      uuid NOT NULL,
  empresa_id   uuid NOT NULL,
  creado_en    timestamptz NOT NULL DEFAULT NOW(),
  PRIMARY KEY (comprador_id, item_id)
);
CREATE INDEX IF NOT EXISTS idx_favoritos_comprador
  ON stores.favoritos (comprador_id, creado_en DESC);

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON stores.resenas TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON stores.favoritos TO core_engine_app;
  END IF;
END $$;

COMMIT;
