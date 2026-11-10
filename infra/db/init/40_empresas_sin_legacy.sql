-- Fase 6: eliminar clasificación legacy de empresas y modalidad logística.
-- La vertical/pack y las capacidades son la única fuente de verdad.
BEGIN;

ALTER TABLE logistics.envios DROP COLUMN IF EXISTS modalidad_logistica;
ALTER TABLE identity.empresas DROP COLUMN IF EXISTS modalidad_logistica;
ALTER TABLE identity.empresas DROP COLUMN IF EXISTS tipo_empresa;

COMMIT;
