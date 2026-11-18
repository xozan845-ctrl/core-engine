-- Compra verificada en reseñas (opt-in por negocio).
--
-- La capacidad `resenas_verificadas` activa la exigencia de una orden pagada
-- para publicar una reseña. Si no está activa, el negocio sigue aceptando
-- reseñas abiertas como hasta ahora.
BEGIN;

ALTER TABLE stores.resenas
  ADD COLUMN IF NOT EXISTS compra_verificada boolean NOT NULL DEFAULT false,
  ADD COLUMN IF NOT EXISTS order_id uuid;

INSERT INTO identity.capacidades (codigo, nombre, modulo)
VALUES (
  'resenas_verificadas',
  'Reseñas con compra verificada',
  'tienda'
)
ON CONFLICT (codigo) DO UPDATE SET nombre = EXCLUDED.nombre, modulo = EXCLUDED.modulo;

-- Disponible en todas las verticales; el negocio decide si la activa.
INSERT INTO identity.vertical_capacidades
  (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'resenas_verificadas', '{}'::jsonb, true
  FROM identity.verticales v
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'core_engine_app') THEN
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.capacidades TO core_engine_app;
    GRANT SELECT, INSERT, UPDATE, DELETE ON identity.vertical_capacidades TO core_engine_app;
  END IF;
END $$;

COMMIT;
