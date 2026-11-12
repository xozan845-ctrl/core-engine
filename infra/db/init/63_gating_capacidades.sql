-- Fase P0.3 · politica de gating (R-AR-5).
-- Separa los servicios siempre activos (identity, gateway, storage,
-- notifications) de los servicios de feature, que deben declarar capacidad.
-- `operacion_campo` gatea field-service y `inteligencia_mercado` gatea
-- market-intelligence-service.
BEGIN;

INSERT INTO identity.capacidades (codigo, nombre, modulo) VALUES
  ('operacion_campo', 'Operacion de campo', 'field'),
  ('inteligencia_mercado', 'Inteligencia de mercado', 'intelligence')
ON CONFLICT (codigo) DO NOTHING;

-- `inteligencia_mercado` queda disponible y activa en toda vertical.
INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'inteligencia_mercado', '{}'::jsonb, true
  FROM identity.verticales v
 WHERE v.activo = true
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

-- `operacion_campo` queda disponible como add-on en toda vertical, pero solo
-- se activa por defecto en flota. Se guarda inactiva para que el empresario
-- pueda habilitarla sin crear un pack nuevo.
INSERT INTO identity.vertical_capacidades (vertical_codigo, capacidad_codigo, config, activo)
SELECT v.codigo, 'operacion_campo', '{}'::jsonb, (v.codigo = 'flota')
  FROM identity.verticales v
 WHERE v.activo = true
ON CONFLICT (vertical_codigo, capacidad_codigo) DO NOTHING;

-- La analitica es transversal a cualquier rubro: los packs activos la incluyen.
INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'inteligencia_mercado', '{}'::jsonb, true
  FROM identity.packs p
 WHERE p.activo = true
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO NOTHING;

-- La operacion de campo es opcional: solo el pack de flota la trae por defecto.
-- Otros rubros pueden activarla como add-on desde la vertical.
INSERT INTO identity.pack_capacidades (pack_codigo, pack_version, capacidad_codigo, config, activo)
SELECT p.codigo, p.version, 'operacion_campo', '{}'::jsonb, true
  FROM identity.packs p
 WHERE p.activo = true AND p.vertical_codigo = 'flota'
ON CONFLICT (pack_codigo, pack_version, capacidad_codigo) DO NOTHING;

-- Backfill: las empresas existentes conservan el acceso que tenian antes del
-- gating. Las nuevas empresas dependen de su pack (configuracion explicita).
INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT e.id, c.codigo, '{}'::jsonb, true
  FROM identity.empresas e
 CROSS JOIN (VALUES ('operacion_campo'), ('inteligencia_mercado')) AS c(codigo)
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

COMMIT;
