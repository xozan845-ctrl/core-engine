-- Fase 8 · capacidad de fidelización (add-on seleccionable).
-- Se ofrece en todas las verticales activas como add-on (inactiva por defecto)
-- y en los packs como capacidad disponible. El alta de empresa puede activarla
-- con `capacidades_extra`.
BEGIN;

INSERT INTO identity.capacidades (codigo, nombre, modulo)
VALUES ('fidelizacion', 'Fidelización', 'loyalty')
ON CONFLICT (codigo) DO UPDATE
SET nombre = EXCLUDED.nombre,
    modulo = EXCLUDED.modulo;

INSERT INTO identity.vertical_capacidades
  (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'fidelizacion', '{}'::jsonb, false
  FROM identity.verticales v
 WHERE v.activo = true
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

INSERT INTO identity.pack_capacidades
  (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'fidelizacion', '{}'::jsonb, false
  FROM identity.packs p
 WHERE p.activo = true
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO NOTHING;

COMMIT;
