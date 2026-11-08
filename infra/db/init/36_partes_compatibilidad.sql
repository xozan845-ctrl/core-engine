-- Plataforma multi-vertical · compatibilidad progresiva con partes (fase 3).
-- Los IDs legacy siguen siendo la referencia primaria durante la transición;
-- parte_id permite adoptar el núcleo universal sin breaking changes.
BEGIN;

ALTER TABLE field.clientes
  ADD COLUMN IF NOT EXISTS parte_id uuid REFERENCES identity.partes(id);
ALTER TABLE field.paradas
  ADD COLUMN IF NOT EXISTS parte_id uuid REFERENCES identity.partes(id);
ALTER TABLE field.pedidos
  ADD COLUMN IF NOT EXISTS parte_id uuid REFERENCES identity.partes(id);
ALTER TABLE field.visitas
  ADD COLUMN IF NOT EXISTS parte_id uuid REFERENCES identity.partes(id);
ALTER TABLE finance.workspace_registros
  ADD COLUMN IF NOT EXISTS parte_id uuid REFERENCES identity.partes(id);
ALTER TABLE orders.orden_vista
  ADD COLUMN IF NOT EXISTS parte_id uuid REFERENCES identity.partes(id);
ALTER TABLE finance.comprobantes
  ADD COLUMN IF NOT EXISTS parte_id uuid REFERENCES identity.partes(id);
ALTER TABLE scheduling.citas
  ADD COLUMN IF NOT EXISTS parte_id uuid REFERENCES identity.partes(id);
ALTER TABLE logistics.envios
  ADD COLUMN IF NOT EXISTS parte_id uuid REFERENCES identity.partes(id);

CREATE INDEX IF NOT EXISTS idx_field_clientes_parte ON field.clientes (tenant_id, parte_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_field_paradas_parte ON field.paradas (tenant_id, parte_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_field_pedidos_parte ON field.pedidos (tenant_id, parte_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_field_visitas_parte ON field.visitas (tenant_id, parte_id) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_workspace_registros_parte ON finance.workspace_registros (tenant_id, parte_id) WHERE eliminado_en IS NULL;
CREATE INDEX IF NOT EXISTS idx_orden_vista_parte ON orders.orden_vista (parte_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_comprobantes_parte ON finance.comprobantes (parte_id, creado_en DESC);
CREATE INDEX IF NOT EXISTS idx_scheduling_citas_parte ON scheduling.citas (empresa_id, parte_id, inicia_en);
CREATE INDEX IF NOT EXISTS idx_logistics_envios_parte ON logistics.envios (empresa_id, parte_id, creado_en DESC);

-- Backfill de clientes de campo: la metadata identifica el origen y permite
-- reejecutar la migración sin duplicar partes.
WITH candidatos AS (
  SELECT c.id AS legacy_id, c.tenant_id AS empresa_id,
         CASE WHEN c.tipo_cliente = 'corporativo' THEN 'organizacion' ELSE 'persona' END AS tipo,
         trim(c.nombre_completo) AS nombre,
         COALESCE(c.tipo_documento, '') AS identificador_tipo,
         COALESCE(c.numero_documento, '') AS identificador_valor,
         jsonb_build_object('origen', 'field.clientes', 'legacy_id', c.id::text) AS metadata,
         COALESCE(e.creado_por, (SELECT em.usuario_id FROM identity.empresa_miembros em
           WHERE em.empresa_id = c.tenant_id AND em.estado = 'activo'
           ORDER BY em.creado_en LIMIT 1)) AS creado_por
    FROM field.clientes c
    JOIN identity.empresas e ON e.id = c.tenant_id
   WHERE c.deleted_at IS NULL AND c.parte_id IS NULL AND trim(c.nombre_completo) <> ''
), nuevos AS (
  INSERT INTO identity.partes
    (empresa_id, tipo, nombre, identificador_tipo, identificador_valor, metadata, creado_por)
  SELECT empresa_id, tipo, nombre, identificador_tipo, identificador_valor, metadata, creado_por
    FROM candidatos
   WHERE creado_por IS NOT NULL
  ON CONFLICT DO NOTHING
  RETURNING id, empresa_id, metadata
)
UPDATE field.clientes c
   SET parte_id = p.id, updated_at = NOW()
  FROM identity.partes p
 WHERE c.parte_id IS NULL
   AND p.empresa_id = c.tenant_id
   AND p.metadata->>'origen' = 'field.clientes'
   AND p.metadata->>'legacy_id' = c.id::text;

INSERT INTO identity.medios_contacto (empresa_id, parte_id, tipo, valor, es_principal)
SELECT c.tenant_id, c.parte_id, 'email', lower(trim(c.email)), true
  FROM field.clientes c
 WHERE c.parte_id IS NOT NULL AND c.deleted_at IS NULL AND trim(COALESCE(c.email, '')) <> ''
ON CONFLICT DO NOTHING;
INSERT INTO identity.medios_contacto (empresa_id, parte_id, tipo, valor, es_principal)
SELECT c.tenant_id, c.parte_id, 'telefono', trim(c.telefono), false
  FROM field.clientes c
 WHERE c.parte_id IS NOT NULL AND c.deleted_at IS NULL AND trim(COALESCE(c.telefono, '')) <> ''
ON CONFLICT DO NOTHING;
INSERT INTO identity.direcciones_partes
  (empresa_id, parte_id, tipo, linea_1, referencias, latitud, longitud, es_principal)
SELECT c.tenant_id, c.parte_id, 'entrega', trim(c.direccion_principal),
       trim(COALESCE(c.referencias_direccion, '')), c.lat, c.lng, true
  FROM field.clientes c
 WHERE c.parte_id IS NOT NULL AND c.deleted_at IS NULL AND trim(COALESCE(c.direccion_principal, '')) <> ''
ON CONFLICT DO NOTHING;

-- Backfill de la cartera de comercio, cuyo contrato legacy vive en JSONB.
WITH candidatos AS (
  SELECT r.id AS legacy_id, r.tenant_id AS empresa_id,
         CASE WHEN trim(COALESCE(r.datos->>'company', '')) <> '' THEN 'organizacion' ELSE 'persona' END AS tipo,
         trim(COALESCE(r.datos->>'name', '')) AS nombre,
         trim(COALESCE(r.datos->>'company', '')) AS nombre_comercial,
         jsonb_build_object('origen', 'finance.workspace_registros:clientes', 'legacy_id', r.id::text) AS metadata,
         e.creado_por
    FROM finance.workspace_registros r
    JOIN identity.empresas e ON e.id = r.tenant_id
   WHERE r.recurso = 'clientes' AND r.eliminado_en IS NULL AND r.parte_id IS NULL
     AND trim(COALESCE(r.datos->>'name', '')) <> ''
), nuevos AS (
  INSERT INTO identity.partes (empresa_id, tipo, nombre, nombre_comercial, metadata, creado_por)
  SELECT empresa_id, tipo, nombre, nombre_comercial, metadata, creado_por
    FROM candidatos
  ON CONFLICT DO NOTHING
  RETURNING id
)
UPDATE finance.workspace_registros r
   SET parte_id = p.id, actualizado_en = NOW()
  FROM identity.partes p
 WHERE r.recurso = 'clientes' AND r.parte_id IS NULL
   AND p.empresa_id = r.tenant_id
   AND p.metadata->>'origen' = 'finance.workspace_registros:clientes'
   AND p.metadata->>'legacy_id' = r.id::text;

INSERT INTO identity.medios_contacto (empresa_id, parte_id, tipo, valor, es_principal)
SELECT r.tenant_id, r.parte_id, 'email', lower(trim(r.datos->>'email')), true
  FROM finance.workspace_registros r
 WHERE r.recurso = 'clientes' AND r.parte_id IS NOT NULL AND r.eliminado_en IS NULL
   AND trim(COALESCE(r.datos->>'email', '')) <> ''
ON CONFLICT DO NOTHING;

-- La proyección de órdenes sigue siendo propiedad del event store. Este
-- trigger agrega la referencia universal sin reemplazar la función existente
-- que materializa el resto de la vista.
CREATE OR REPLACE FUNCTION orders.fn_sync_order_part() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'INSERT'
     AND NEW.tipo = 'OrderCreatedEvent'
     AND NEW.payload ? 'cliente_parte_id'
     AND (NEW.payload->>'cliente_parte_id') ~* '^[0-9a-f-]{36}$' THEN
    UPDATE orders.orden_vista
       SET parte_id = (NEW.payload->>'cliente_parte_id')::uuid,
           actualizado_en = NOW()
     WHERE id = NEW.aggregate_id;
  END IF;
  RETURN NEW;
END;
$$;

DROP TRIGGER IF EXISTS zz_order_part_projection ON orders.eventos;
CREATE TRIGGER zz_order_part_projection
AFTER INSERT ON orders.eventos
FOR EACH ROW EXECUTE FUNCTION orders.fn_sync_order_part();

-- El replay histórico invoca fn_aplicar_evento directamente y, por tanto, no
-- pasa por triggers. Se conserva la misma reconstrucción y se sincroniza la
-- referencia universal desde el payload original.
CREATE OR REPLACE FUNCTION orders.fn_bootstrap_proyecciones()
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  r RECORD;
BEGIN
  TRUNCATE orders.orden_vista, orders.orden_timeline, orders.comisiones_vista;
  FOR r IN SELECT * FROM orders.eventos ORDER BY aggregate_id, version LOOP
    PERFORM orders.fn_aplicar_evento(r.event_id, r.aggregate_id, r.tipo, r.payload, r.version, r.creado_en);
    IF r.tipo = 'OrderCreatedEvent'
       AND r.payload ? 'cliente_parte_id'
       AND (r.payload->>'cliente_parte_id') ~* '^[0-9a-f-]{36}$' THEN
      UPDATE orders.orden_vista
         SET parte_id = (r.payload->>'cliente_parte_id')::uuid,
             actualizado_en = NOW()
       WHERE id = r.aggregate_id;
    END IF;
  END LOOP;
END;
$$;
INSERT INTO identity.medios_contacto (empresa_id, parte_id, tipo, valor, es_principal)
SELECT r.tenant_id, r.parte_id, 'telefono', trim(r.datos->>'phone'), false
  FROM finance.workspace_registros r
 WHERE r.recurso = 'clientes' AND r.parte_id IS NOT NULL AND r.eliminado_en IS NULL
   AND trim(COALESCE(r.datos->>'phone', '')) <> ''
ON CONFLICT DO NOTHING;

COMMENT ON COLUMN field.clientes.parte_id IS 'Referencia universal opcional; cliente_id legacy se conserva durante la transición.';
COMMENT ON COLUMN finance.workspace_registros.parte_id IS 'Referencia universal opcional para recurso clientes; datos JSON legacy se conserva.';
COMMENT ON COLUMN orders.orden_vista.parte_id IS 'Referencia universal opcional del comprador por empresa.';
COMMENT ON COLUMN finance.comprobantes.parte_id IS 'Referencia universal opcional del cliente fiscal.';
COMMENT ON COLUMN scheduling.citas.parte_id IS 'Referencia universal opcional del cliente; cliente_id legacy se conserva.';
COMMENT ON COLUMN logistics.envios.parte_id IS 'Referencia universal opcional del destinatario; order_id se conserva.';

COMMIT;
