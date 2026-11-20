-- Prueba de entrega (POD) mínima para la vertical flota/logística (ADR-0043).
BEGIN;

ALTER TABLE logistics.envios
  ADD COLUMN IF NOT EXISTS pod_receptor text,
  ADD COLUMN IF NOT EXISTS pod_notas text,
  ADD COLUMN IF NOT EXISTS pod_foto_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS pod_firma_objeto_id uuid,
  ADD COLUMN IF NOT EXISTS pod_capturado_en timestamptz;

INSERT INTO identity.capacidades (codigo, nombre, modulo) VALUES
  ('logistica_pod', 'Prueba de entrega', 'logistics')
ON CONFLICT (codigo) DO NOTHING;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
VALUES ('flota', 'logistica_pod', '{"requerido":true}'::jsonb, true)
ON CONFLICT (vertical_codigo, capacidad_codigo) DO UPDATE
  SET config = EXCLUDED.config, activo = EXCLUDED.activo;

INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT 'pack_' || vc.vertical_codigo, 1, vc.capacidad_codigo, vc.config, vc.activo
  FROM identity.vertical_capacidades vc
 WHERE vc.vertical_codigo = 'flota' AND vc.capacidad_codigo = 'logistica_pod'
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO UPDATE
  SET config = EXCLUDED.config, activo = EXCLUDED.activo;

COMMIT;
