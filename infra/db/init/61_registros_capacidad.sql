-- Fase E · capacidad genérica de registros (R-AR-5).
-- Desacopla el módulo de registros de la nomenclatura clínica: los tipos
-- operativos (diagnóstico, inspección, expediente, nota) usan `registros`;
-- `historia_clinica` y `recetas` siguen siendo específicos de salud.
BEGIN;

INSERT INTO identity.capacidades (codigo, nombre, modulo)
VALUES ('registros', 'Registros operativos', 'records')
ON CONFLICT (codigo) DO NOTHING;

INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'registros', '{}'::jsonb, true
  FROM identity.verticales v
 WHERE v.activo = true
ON CONFLICT (vertical_codigo, capacidad_codigo) DO UPDATE
  SET activo = true, config = EXCLUDED.config;

INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'registros', '{}'::jsonb, true
  FROM identity.packs p
 WHERE p.activo = true
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO UPDATE
  SET activo = true, config = EXCLUDED.config;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, 'registros', '{}'::jsonb, true
  FROM identity.empresas e
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

COMMIT;
