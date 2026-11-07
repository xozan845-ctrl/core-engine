-- Fase 6: asignacion progresiva del pack a tenants existentes.
-- No borra datos ni reescribe capacidades; solo completa el pack derivado de
-- la vertical y deja una auditoria por empresa para operar el backfill.
BEGIN;

CREATE TABLE IF NOT EXISTS identity.pack_migraciones (
  empresa_id              uuid PRIMARY KEY REFERENCES identity.empresas(id) ON DELETE CASCADE,
  vertical_origen         text,
  tipo_empresa_origen     text,
  modalidad_origen        text,
  pack_codigo             text,
  pack_version            integer,
  capacidades_antes       integer NOT NULL DEFAULT 0 CHECK (capacidades_antes >= 0),
  capacidades_despues     integer NOT NULL DEFAULT 0 CHECK (capacidades_despues >= 0),
  estado                  text NOT NULL CHECK (estado IN ('aplicado', 'requiere_revision')),
  detalles                jsonb NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(detalles) = 'object'),
  migrado_en              timestamptz NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_pack_migraciones_estado
  ON identity.pack_migraciones (estado, migrado_en);

CREATE TEMP TABLE _pack_backfill_candidates ON COMMIT DROP AS
SELECT e.id AS empresa_id,
       e.vertical_id AS vertical_origen,
       e.tipo_empresa AS tipo_empresa_origen,
       e.modalidad_logistica AS modalidad_origen,
       p.codigo AS pack_codigo,
       p.version AS pack_version,
       (SELECT COUNT(*)::integer
          FROM identity.empresa_capacidades ec
         WHERE ec.empresa_id = e.id) AS capacidades_antes
  FROM identity.empresas e
  LEFT JOIN identity.packs p
    ON p.codigo = 'pack_' || e.vertical_id
   AND p.version = 1
   AND p.activo = true
 WHERE e.pack_codigo IS NULL;

UPDATE identity.empresas e
   SET pack_codigo = c.pack_codigo,
       pack_version = c.pack_version
  FROM _pack_backfill_candidates c
 WHERE e.id = c.empresa_id
   AND c.pack_codigo IS NOT NULL;

INSERT INTO identity.empresa_capacidades (empresa_id, capacidad_codigo, config, activo)
SELECT c.empresa_id, pc.capacidad_codigo, pc.config, pc.activo
  FROM _pack_backfill_candidates c
  JOIN identity.pack_capacidades pc
    ON pc.pack_codigo = c.pack_codigo
   AND pc.pack_version = c.pack_version
 WHERE c.pack_codigo IS NOT NULL
ON CONFLICT (empresa_id, capacidad_codigo) DO NOTHING;

INSERT INTO identity.pack_migraciones (
  empresa_id, vertical_origen, tipo_empresa_origen, modalidad_origen,
  pack_codigo, pack_version, capacidades_antes, capacidades_despues,
  estado, detalles
)
SELECT c.empresa_id,
       c.vertical_origen,
       c.tipo_empresa_origen,
       c.modalidad_origen,
       c.pack_codigo,
       c.pack_version,
       c.capacidades_antes,
       (SELECT COUNT(*)::integer
          FROM identity.empresa_capacidades ec
         WHERE ec.empresa_id = c.empresa_id),
       CASE WHEN c.pack_codigo IS NULL THEN 'requiere_revision' ELSE 'aplicado' END,
       CASE WHEN c.pack_codigo IS NULL
            THEN jsonb_build_object('motivo', 'vertical_sin_pack_activo')
            ELSE jsonb_build_object('origen', 'vertical_id', 'preserva_capacidades', true)
       END
  FROM _pack_backfill_candidates c
ON CONFLICT (empresa_id) DO NOTHING;

COMMIT;
